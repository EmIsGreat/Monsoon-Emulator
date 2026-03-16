use std::thread;

use crossbeam_channel::{Receiver, Sender};
use monsoon_core::emulation::nes::Nes;
use monsoon_core::emulation::ppu_util::{EmulatorFetchable, PaletteData};
use monsoon_core::util::Hashable;

use crate::messages::{ControllerEvent, EmulatorMessage, FrontendMessage, SaveType};

/// Threaded emulator wrapper that runs emulation on a background thread.
///
/// This wrapper creates the `Nes` instance **on the background thread** rather than
/// moving it there, which avoids the `Send` requirement. Since `Nes` contains
/// `Rc<RefCell<Ppu>>` which is not `Send`, this approach allows true multi-threading
/// without requiring expensive `Arc<Mutex<Ppu>>` synchronization.
///
/// # Key Insight
///
/// - `Nes` is **NOT** `Send` due to `Rc<RefCell<Ppu>>`
/// - Creating `Nes` on the background thread means it never crosses thread boundaries
/// - Only channel messages need to be `Send`, which they already are
/// - This avoids the overhead of `Arc<Mutex<>>` while still enabling threaded execution
///
/// # Architecture
///
/// ```text
/// UI Thread                          Background Thread
/// ─────────                          ─────────────────
///   │                                      │
///   ├─ Send FrontendMessage ─────────────>│
///   │   (StepFrame, LoadRom, etc.)        │
///   │                                      ├─ Nes::new() on thread
///   │                                      ├─ process_messages()
///   │                                      ├─ nes.step_frame()
///   │                                      │
///   │<──── Send EmulatorMessage ──────────┤
///        (FrameReady, DebugData, etc.)    │
/// ```
///
/// # Benefits over `ChannelEmulator`
///
/// - UI thread remains responsive during emulation
/// - No Arc/Mutex overhead (keeps Rc<RefCell> performance)
/// - Clean separation of UI and emulation work
/// - Same message-based API as non-threaded version
///
pub struct ThreadedEmulator {
    thread_handle: Option<thread::JoinHandle<()>>,
    to_emulator: Sender<FrontendMessage>,
}

impl ThreadedEmulator {
    /// Creates a new threaded emulator.
    ///
    /// The `Nes` instance is created on the spawned background thread, avoiding
    /// the need for `Nes` to implement `Send`.
    pub fn new() -> (Self, Receiver<EmulatorMessage>) {
        let (tx_to_emu, rx_from_frontend) = crossbeam_channel::unbounded();
        let (tx_from_emu, rx_to_frontend) = crossbeam_channel::unbounded();

        // Clone the sender for use in the thread
        let tx_to_emu_clone = tx_to_emu.clone();

        // Spawn the emulation thread
        let handle = thread::spawn(move || {
            // Create Nes on the background thread (avoids Send requirement)
            let nes = Nes::default();

            let mut worker = EmulatorWorker {
                nes,
                to_frontend: tx_from_emu,
                from_frontend: rx_from_frontend,
                input: 0,
                last_palette_data: None,
                last_pattern_table_hash: None,
                running: true,
            };

            worker.run();
        });

        let threaded_emu = Self {
            thread_handle: Some(handle),
            to_emulator: tx_to_emu_clone,
        };

        (threaded_emu, rx_to_frontend)
    }

    /// Sends a message to the emulator thread.
    pub fn send(&self, msg: FrontendMessage) -> Result<(), String> {
        self.to_emulator
            .send(msg)
            .map_err(|e| format!("Failed to send message: {}", e))
    }
}

impl Drop for ThreadedEmulator {
    fn drop(&mut self) {
        // Send quit message and wait for thread to finish
        let _ = self.to_emulator.send(FrontendMessage::Quit);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Worker that runs on the background thread.
///
/// This struct contains the actual `Nes` instance and processes messages
/// from the frontend. Since it's created and used entirely on one thread,
/// it doesn't need to be `Send`.
struct EmulatorWorker {
    nes: Nes,
    to_frontend: Sender<EmulatorMessage>,
    from_frontend: Receiver<FrontendMessage>,
    input: u8,
    last_palette_data: Option<PaletteData>,
    last_pattern_table_hash: Option<u64>,
    running: bool,
}

impl EmulatorWorker {
    /// Main loop for the emulator thread.
    fn run(&mut self) {
        while self.running {
            // Process all pending messages
            match self.process_messages() {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Emulator error: {}", e);
                    break;
                }
            }

            // Small sleep to prevent busy-waiting when no messages
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
    }

    /// Process messages from the frontend.
    fn process_messages(&mut self) -> Result<(), String> {
        // Process all available messages
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
                    self.running = false;
                    return Ok(());
                }
                FrontendMessage::Reset => {
                    self.nes.reset();
                }
                FrontendMessage::StepFrame => {
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

    fn execute_frame(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step_frame() {
            Ok(_) => {
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    return Err("Frontend disconnected".to_string());
                }

                self.check_debug_data_changed();
                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    fn execute_master_cycle(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step() {
            Ok(_) => {
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    return Err("Frontend disconnected".to_string());
                }

                self.check_debug_data_changed();
                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    fn execute_ppu_cycle(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step_ppu_cycle() {
            Ok(_) => {
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    return Err("Frontend disconnected".to_string());
                }

                self.check_debug_data_changed();
                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    fn execute_cpu_cycle(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step_cpu_cycle() {
            Ok(_) => {
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    return Err("Frontend disconnected".to_string());
                }

                self.check_debug_data_changed();
                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    fn execute_scanline(&mut self) -> Result<(), String> {
        self.nes.cpu_mem_init(0x4016, self.input);
        self.input = 0;

        match self.nes.step_scanline() {
            Ok(_) => {
                let frame = self.nes.get_pixel_buffer();
                let frame_data = (*frame).to_vec();
                if self
                    .to_frontend
                    .send(EmulatorMessage::FrameReady(frame_data))
                    .is_err()
                {
                    return Err("Frontend disconnected".to_string());
                }

                self.check_debug_data_changed();
                Ok(())
            }
            Err(e) => Err(format!("Emulator error: {}", e)),
        }
    }

    fn check_debug_data_changed(&mut self) {
        // Check palette data
        if let EmulatorFetchable::Palettes(Some(current_palette)) = self.nes.get_palettes_debug() {
            let current = *current_palette;
            let palette_changed = match &self.last_palette_data {
                Some(last) => *last != current,
                None => true,
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

        // Check tile/pattern table data
        let pattern_table_memory = self.nes.get_memory_debug(Some(0x0000..=0x1FFF))[1].to_vec();
        let current_hash = &pattern_table_memory.hash();

        let tiles_changed = match self.last_pattern_table_hash {
            Some(last_hash) => last_hash != *current_hash,
            None => true,
        };

        if tiles_changed {
            self.last_pattern_table_hash = Some(*current_hash);
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
}
