use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crossbeam_channel::{Receiver, Sender};
use monsoon_core::emulation::nes::Nes;
use monsoon_core::emulation::ppu_util::{EmulatorFetchable, PaletteData};
use monsoon_core::util::Hashable;

use crate::messages::{ControllerEvent, EmulatorMessage, FrontendMessage, SaveType};

/// Channel-based emulator wrapper for clean frontend/emulator separation.
///
/// This module provides a non-threaded emulator wrapper that uses channels for
/// communication. While it doesn't provide true multi-threading (due to the
/// emulator core using `Rc<RefCell<>>` which is not `Send`), it provides:
///
/// - Clean separation of concerns between frontend and emulator
/// - Message-based communication protocol
/// - Easy upgrade path to multi-threading once core is refactored
/// - Testable architecture
///
/// # Architecture
///
/// ```text
/// Frontend → FrontendMessage → ChannelEmulator → Emulator Core
///              (channels)              ↓
/// Frontend ← EmulatorMessage ←─────────┘
/// ```
///
/// # Example
///
/// ```ignore
/// use lockstep::emulation::channel_emu::ChannelEmulator;
/// use lockstep::emulation::emu::{Console, Consoles};
/// use lockstep::emulation::nes::Nes;
///
/// let console = Nes::default();
/// let (mut emu, tx_to_emu, rx_from_emu) = ChannelEmulator::new(console);
///
/// // In your main loop:
/// emu.step_frame()?; // Run one frame
/// ```
/// A non-threaded emulator wrapper that communicates via channels
/// This provides a clean interface for the frontend without threading complications.
/// The emulator runs in the same thread but is decoupled via message passing.
pub struct ChannelEmulator {
    pub nes: Nes,
    to_frontend: Sender<EmulatorMessage>,
    from_frontend: Receiver<FrontendMessage>,
    input: u8,
    /// Cached palette data for change detection
    last_palette_data: Option<PaletteData>,
    /// Cached hash of pattern table data for efficient change detection
    last_pattern_table_hash: Option<u64>,
}

pub static FETCH_DEPS: OnceLock<HashMap<EmulatorFetchable, Vec<EmulatorFetchable>>> =
    OnceLock::new();

impl ChannelEmulator {
    fn setup_fetch_deps() {
        let mut deps = HashMap::new();

        deps.insert(
            EmulatorFetchable::Tiles(None),
            vec![EmulatorFetchable::Palettes(None)],
        );
        deps.insert(
            EmulatorFetchable::Nametables(None),
            vec![EmulatorFetchable::Tiles(None)],
        );
        deps.insert(
            EmulatorFetchable::Sprites(None),
            vec![EmulatorFetchable::Tiles(None)],
        );

        deps.insert(
            EmulatorFetchable::SoamSprites(None),
            vec![EmulatorFetchable::Tiles(None)],
        );

        FETCH_DEPS.get_or_init(|| deps);
    }

    pub fn new(nes: Nes) -> (Self, Sender<FrontendMessage>, Receiver<EmulatorMessage>) {
        Self::setup_fetch_deps();

        let (tx_to_emu, rx_from_frontend) = crossbeam_channel::unbounded();
        let (tx_from_emu, rx_to_frontend) = crossbeam_channel::unbounded();

        let emu = Self {
            nes,
            to_frontend: tx_from_emu,
            from_frontend: rx_from_frontend,
            input: 0,
            last_palette_data: None,
            last_pattern_table_hash: None,
        };

        (emu, tx_to_emu, rx_to_frontend)
    }

    /// Run one frame of emulation and handle messages
    pub fn process_messages(&mut self) -> Result<(), String> {
        // Check for messages from frontend
        while let Ok(msg) = self.from_frontend.try_recv() {
            match msg {
                FrontendMessage::Quit => {
                    let state = self.nes.save_state();
                    if let Some(state) = state {
                        let _ = self.to_frontend.send(EmulatorMessage::SaveState(
                            Box::new(state),
                            SaveType::Autosave,
                        ));
                    }
                    let _ = self.to_frontend.send(EmulatorMessage::Stopped);
                    return Err("Quit requested".to_string());
                }
                FrontendMessage::Reset => {
                    self.nes.reset();
                }
                FrontendMessage::StepFrame => {
                    // Execute one frame regardless of pause state
                    self.execute_frame()?;
                }
                FrontendMessage::ControllerInput(event) => {
                    self.handle_controller_event(event);
                }
                FrontendMessage::RequestDebugData(fetchable) => match fetchable {
                    EmulatorFetchable::Palettes(_) => {
                        let _ = self
                            .to_frontend
                            .send(EmulatorMessage::DebugData(self.nes.get_palettes_debug()));
                    }
                    EmulatorFetchable::Tiles(_) => {
                        let _ = self
                            .to_frontend
                            .send(EmulatorMessage::DebugData(self.nes.get_tiles_debug()));
                    }
                    EmulatorFetchable::Nametables(_) => {
                        let _ = self
                            .to_frontend
                            .send(EmulatorMessage::DebugData(self.nes.get_nametable_debug()));
                    }
                    EmulatorFetchable::Sprites(_) => {
                        let _ = self
                            .to_frontend
                            .send(EmulatorMessage::DebugData(self.nes.get_sprites_debug()));
                    }
                    EmulatorFetchable::SoamSprites(_) => {
                        let _ = self.to_frontend.send(EmulatorMessage::DebugData(
                            self.nes.get_soam_sprites_debug(),
                        ));
                    }
                },
                FrontendMessage::WritePpu(address, data) => self.nes.ppu_mem_init(address, data),
                FrontendMessage::WriteCpu(address, data) => self.nes.cpu_mem_init(address, data),
                FrontendMessage::LoadRom((rom, name)) => {
                    let loadable = (&rom.data[..], name);
                    self.nes.load_rom(&loadable);
                    let _ = self.to_frontend.send(EmulatorMessage::RomLoaded(
                        self.nes.rom_file.clone().map(|r| (r, rom)),
                    ));
                }
                FrontendMessage::Power => {
                    self.nes.power();
                }
                FrontendMessage::PowerOff => self.nes.power_off(),
                FrontendMessage::CreateSaveState(t) => {
                    if self.nes.rom_file.is_some() {
                        let state = self.nes.save_state();
                        if let Some(state) = state {
                            let _ = self
                                .to_frontend
                                .send(EmulatorMessage::SaveState(Box::new(state), t));
                        }
                    }
                }
                FrontendMessage::LoadSaveState(s) => self.nes.load_state(*s),
                FrontendMessage::StepPpuCycle => self.execute_ppu_cycle()?,
                FrontendMessage::StepCpuCycle => self.execute_cpu_cycle()?,
                FrontendMessage::StepMasterCycle => self.execute_master_cycle()?,
                FrontendMessage::StepScanline => self.execute_scanline()?,
            }
        }

        Ok(())
    }

    pub fn execute_master_cycle(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step() {
            Ok(_) => {
                // Frame completed, send it to frontend
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    // Frontend disconnected
                    return Err("Frontend disconnected".to_string());
                }

                // Check if debug data has changed and notify frontend
                self.check_debug_data_changed();

                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    pub fn execute_ppu_cycle(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step_ppu_cycle() {
            Ok(_) => {
                // Frame completed, send it to frontend
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    // Frontend disconnected
                    return Err("Frontend disconnected".to_string());
                }

                // Check if debug data has changed and notify frontend
                self.check_debug_data_changed();

                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    pub fn execute_cpu_cycle(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step_cpu_cycle() {
            Ok(_) => {
                // Frame completed, send it to frontend
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    // Frontend disconnected
                    return Err("Frontend disconnected".to_string());
                }

                // Check if debug data has changed and notify frontend
                self.check_debug_data_changed();

                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    pub fn execute_scanline(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step_scanline() {
            Ok(_) => {
                // Frame completed, send it to frontend
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    // Frontend disconnected
                    return Err("Frontend disconnected".to_string());
                }

                // Check if debug data has changed and notify frontend
                self.check_debug_data_changed();

                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    pub fn execute_frame(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step_frame() {
            Ok(_) => {
                // Frame completed, send it to frontend
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    // Frontend disconnected
                    return Err("Frontend disconnected".to_string());
                }

                // Check if debug data has changed and notify frontend
                self.check_debug_data_changed();

                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    /// Check if debug data has changed since last check, and send the data if so.
    /// This enables passive fetching of debug data - the frontend only rebuilds
    /// textures when data actually changes, rather than on a regular interval.
    fn check_debug_data_changed(&mut self) {
        // Check palette data (32 bytes, cheap comparison)
        if let EmulatorFetchable::Palettes(Some(current_palette)) = self.nes.get_palettes_debug() {
            let current = *current_palette; // Copy the PaletteData (it's 32 bytes)
            let palette_changed = match &self.last_palette_data {
                Some(last) => *last != current,
                None => true, // First time, consider it changed
            };

            if palette_changed {
                self.last_palette_data = Some(current);
                let _ =
                    self.to_frontend
                        .send(EmulatorMessage::DebugData(EmulatorFetchable::Palettes(
                            Some(Box::new(current)),
                        )));
            }
        }

        // Check tile/pattern table data using a fast hash of raw PPU memory
        // Pattern tables occupy 0x0000-0x1FFF (8KB) in PPU address space
        let pattern_table_memory = self.nes.get_memory_debug(Some(0x0000..=0x1FFF))[1].to_vec();
        let current_hash = &pattern_table_memory.hash();

        let tiles_changed = match self.last_pattern_table_hash {
            Some(last_hash) => last_hash != *current_hash,
            None => true, // First time, consider it changed
        };

        if tiles_changed {
            self.last_pattern_table_hash = Some(*current_hash);
            // Send the actual tile data directly to avoid a round-trip request
            let _ = self
                .to_frontend
                .send(EmulatorMessage::DebugData(self.nes.get_tiles_debug()));
        }
    }

    fn handle_controller_event(&mut self, event: ControllerEvent) {
        match event {
            ControllerEvent::A => self.input |= 0x1,
            ControllerEvent::B => self.input |= 0x2,
            ControllerEvent::Select => self.input |= 0x4,
            ControllerEvent::Start => self.input |= 0x8,
            ControllerEvent::Up => self.input |= 0x10,
            ControllerEvent::Down => self.input |= 0x20,
            ControllerEvent::Left => self.input |= 0x40,
            ControllerEvent::Right => self.input |= 0x80,
        }
    }

    pub fn compute_required_fetches(
        enabled: &HashSet<EmulatorFetchable>,
        deps: &HashMap<EmulatorFetchable, Vec<EmulatorFetchable>>,
    ) -> HashSet<EmulatorFetchable> {
        let mut fetch = HashSet::new();
        let mut stack: Vec<_> = Vec::with_capacity(enabled.len());

        for x in enabled.iter() {
            stack.push(EmulatorFetchable::get_empty(x));
        }

        while let Some(to_fetch) = stack.pop() {
            // Only process if we haven't seen this fetchable before
            if fetch.insert(EmulatorFetchable::get_empty(&to_fetch)) {
                // If this fetchable has dependencies, add them to the stack for processing
                if let Some(reqs) = deps.get(&to_fetch) {
                    for x in reqs {
                        let empty = EmulatorFetchable::get_empty(x);
                        // Only add to stack if not already in fetch set
                        if !fetch.contains(&empty) {
                            stack.push(empty);
                        }
                    }
                }
            }
        }

        fetch
    }
}
