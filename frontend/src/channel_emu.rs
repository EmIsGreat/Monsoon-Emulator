use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crossbeam_channel::{Receiver, Sender};
use monsoon_core::emulation::nes::Nes;
use monsoon_core::emulation::ppu_util::{
    EmulatorFetchable, PaletteData, TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH,
};
use monsoon_core::emulation::rom::{ParseError, RomFile};
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
/// This provides a clean interface for the frontend without threading
/// complications. The emulator runs in the same thread but is decoupled via
/// message passing.
pub struct ChannelEmulator {
    pub nes: Nes,
    to_frontend: Sender<EmulatorMessage>,
    from_frontend: Receiver<FrontendMessage>,
    /// Triple-buffer *back* buffer: holds the most recently completed frame.
    ///
    /// On each frame boundary the emulator swaps this with the PPU's internal
    /// *work* buffer (zero-copy). The frontend then swaps this buffer with its
    /// own *front* buffer before rendering, also without copying.
    pub back_buffer: Vec<u16>,
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

    #[must_use]
    pub fn new(nes: Nes) -> (Self, Sender<FrontendMessage>, Receiver<EmulatorMessage>) {
        Self::setup_fetch_deps();

        let (tx_to_emu, rx_from_frontend) = crossbeam_channel::unbounded();
        let (tx_from_emu, rx_to_frontend) = crossbeam_channel::unbounded();

        let emu = Self {
            nes,
            to_frontend: tx_from_emu,
            from_frontend: rx_from_frontend,
            back_buffer: vec![
                0u16;
                usize::from(TOTAL_OUTPUT_HEIGHT) * usize::from(TOTAL_OUTPUT_WIDTH)
            ],
            last_palette_data: None,
            last_pattern_table_hash: None,
        };

        (emu, tx_to_emu, rx_to_frontend)
    }

    /// Run one frame of emulation and handle messages
    pub fn process_messages(&mut self) -> Result<(), String> {
        // Controller input is provided by frontend each UI update for bindings
        // currently held. Reset the bitfield once per update so held inputs
        // persist across all frames executed this update, but release cleanly
        // when not re-sent on the next update.

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
                FrontendMessage::ControllerInput(event, is_slot_one) => {
                    self.handle_controller_event(event, is_slot_one);
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
                    EmulatorFetchable::Registers(_) => {
                        let _ = self
                            .to_frontend
                            .send(EmulatorMessage::DebugData(self.nes.get_registers_debug()));
                    }
                },
                FrontendMessage::WritePpu(address, data) => self.nes.ppu_mem_init(address, data),
                FrontendMessage::WriteCpu(address, data) => self.nes.cpu_mem_init(address, data),
                FrontendMessage::LoadRom((mut rom, name, use_db)) => {
                    let loadable = (&mut rom.data[..], &name, use_db, Some(&self.nes));
                    let rom_file: Result<RomFile, ParseError> = loadable.try_into();

                    if let Ok(rom_file) = rom_file {
                        let _ = self.nes.load_rom(&rom_file);

                        let _ = self.to_frontend.send(EmulatorMessage::RomLoaded(Box::new(
                            self.nes.rom_file.clone().map(|r| (r, rom)),
                        )));
                    }
                }
                FrontendMessage::Power(is_powered) => {
                    if is_powered {
                        self.nes.power();
                    } else {
                        self.nes.power_off();
                    }
                }
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
                FrontendMessage::AttachPeripherals((peripheral1, peripheral2)) => {
                    self.nes.attach_ext_device((peripheral1, peripheral2));
                }
                FrontendMessage::UpdateRomDb(db) => self.nes.rom_db = db,
            }
        }

        Ok(())
    }

    pub fn execute_master_cycle(&mut self) -> Result<(), String> {
        self.nes.step();

        // Copy (not swap) so the PPU's accumulated mid-frame render is
        // preserved in the work buffer for subsequent debug steps.
        self.back_buffer
            .copy_from_slice(self.nes.get_pixel_buffer());
        if self.to_frontend.send(EmulatorMessage::FrameReady).is_err() {
            return Err("Frontend disconnected".to_string());
        }

        // Check if debug data has changed and notify frontend
        self.check_debug_data_changed();

        Ok(())
    }

    pub fn execute_ppu_cycle(&mut self) -> Result<(), String> {
        self.nes.step_ppu_cycle();
        // Copy (not swap) so the PPU's accumulated mid-frame render is
        // preserved in the work buffer for subsequent debug steps.
        self.back_buffer
            .copy_from_slice(self.nes.get_pixel_buffer());
        if self.to_frontend.send(EmulatorMessage::FrameReady).is_err() {
            return Err("Frontend disconnected".to_string());
        }

        // Check if debug data has changed and notify frontend
        self.check_debug_data_changed();

        Ok(())
    }

    pub fn execute_cpu_cycle(&mut self) -> Result<(), String> {
        self.nes.step_cpu_cycle();
        // Copy (not swap) so the PPU's accumulated mid-frame render is
        // preserved in the work buffer for subsequent debug steps.
        self.back_buffer
            .copy_from_slice(self.nes.get_pixel_buffer());
        if self.to_frontend.send(EmulatorMessage::FrameReady).is_err() {
            return Err("Frontend disconnected".to_string());
        }

        // Check if debug data has changed and notify frontend
        self.check_debug_data_changed();

        Ok(())
    }

    pub fn execute_scanline(&mut self) -> Result<(), String> {
        self.nes.step_scanline();
        // Copy (not swap) so the PPU's accumulated mid-frame render is
        // preserved in the work buffer for subsequent debug steps.
        self.back_buffer
            .copy_from_slice(self.nes.get_pixel_buffer());
        if self.to_frontend.send(EmulatorMessage::FrameReady).is_err() {
            return Err("Frontend disconnected".to_string());
        }

        // Check if debug data has changed and notify frontend
        self.check_debug_data_changed();

        Ok(())
    }

    pub fn execute_frame(&mut self) -> Result<(), String> {
        self.nes.step_frame();
        // Swap the PPU work buffer with the back buffer (zero-copy).
        self.nes.swap_pixel_buffer(&mut self.back_buffer);
        if self.to_frontend.send(EmulatorMessage::FrameReady).is_err() {
            return Err("Frontend disconnected".to_string());
        }

        // Check if debug data has changed and notify frontend
        self.check_debug_data_changed();

        Ok(())
    }

    /// Check if debug data has changed since last check, and send the data if
    /// so. This enables passive fetching of debug data - the frontend only
    /// rebuilds textures when data actually changes, rather than on a regular
    /// interval.
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
        let pattern_table_memory = self.nes.get_memory_debug(&Some(0x0000..=0x1FFF))[1].clone();
        let current_hash = pattern_table_memory.hash();

        let Ok(current_hash) = current_hash else {
            return;
        };

        let tiles_changed = if let Some(last_hash) = self.last_pattern_table_hash {
            current_hash != last_hash
        } else {
            true
        };

        if tiles_changed {
            self.last_pattern_table_hash = Some(current_hash);
            // Send the actual tile data directly to avoid a round-trip request
            let _ = self
                .to_frontend
                .send(EmulatorMessage::DebugData(self.nes.get_tiles_debug()));
        }
    }

    fn handle_controller_event(&mut self, event: ControllerEvent, is_slot_one: bool) {
        // self.nes.set_controller_input()
        //
        // let input_field = if is_slot_one {
        //     &mut self.input_1
        // } else {
        //     &mut self.input_2
        // };
        //
        // match event {
        //     ControllerEvent::StdA => *input_field |= 0x1,
        //     ControllerEvent::StdB => *input_field |= 0x2,
        //     ControllerEvent::StdSelect => *input_field |= 0x4,
        //     ControllerEvent::StdStart => *input_field |= 0x8,
        //     ControllerEvent::StdUp => *input_field |= 0x10,
        //     ControllerEvent::StdDown => *input_field |= 0x20,
        //     ControllerEvent::StdLeft => *input_field |= 0x40,
        //     ControllerEvent::StdRight => *input_field |= 0x80,
        // }
    }

    #[must_use]
    pub fn compute_required_fetches(
        enabled: &HashSet<EmulatorFetchable>,
        deps: &HashMap<EmulatorFetchable, Vec<EmulatorFetchable>>,
    ) -> HashSet<EmulatorFetchable> {
        let mut fetch = HashSet::new();
        let mut stack: Vec<_> = Vec::with_capacity(enabled.len());

        for x in enabled {
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
