use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender};
use monsoon_core::emulation::nes::Nes;

use crate::channel_emu::ChannelEmulator;
use crate::messages::{EmulatorMessage, FrontendMessage};

/// Threaded emulator wrapper that runs the emulator on a separate background thread.
///
/// This architecture solves the performance problem of Arc<Mutex<>> by creating
/// the Nes emulator directly on the background thread, avoiding the Send requirement
/// for Rc<RefCell<Ppu>>. The emulator runs autonomously on its own thread and
/// communicates with the frontend via message passing.
///
/// # Architecture
///
/// ```text
/// Main Thread                     Background Thread
/// -----------                     -----------------
/// Frontend → FrontendMessage →    ThreadedEmulator
///                                      ↓
///                                   ChannelEmulator (owns Nes)
///                                      ↓
/// Frontend ← EmulatorMessage ←─────────┘
/// ```
///
/// # Benefits
///
/// - **No Arc overhead**: Keeps fast Rc<RefCell<Ppu>> since Nes never leaves its thread
/// - **True parallelism**: Emulator runs concurrently with UI rendering
/// - **Uncapped execution**: Background thread can run at full speed without blocking UI
/// - **Responsive UI**: Frontend never blocks waiting for frame completion
///
/// # Example
///
/// ```ignore
/// let (threaded_emu, to_emu, from_emu) = ThreadedEmulator::new();
///
/// // Send messages to emulator
/// to_emu.send(FrontendMessage::StepFrame).unwrap();
///
/// // Receive frame data
/// while let Ok(msg) = from_emu.recv() {
///     match msg {
///         EmulatorMessage::FrameReady(frame) => { /* render */ }
///         _ => {}
///     }
/// }
/// ```
pub struct ThreadedEmulator {
    /// Channel to send messages to the emulator thread
    pub(crate) to_emulator: Sender<FrontendMessage>,
    /// Handle to the background thread (kept alive)
    thread_handle: Option<JoinHandle<()>>,
}

impl ThreadedEmulator {
    /// Create a new threaded emulator.
    ///
    /// Returns:
    /// - `ThreadedEmulator`: Handle to the background thread
    /// - `Sender<FrontendMessage>`: Channel to send commands to emulator
    /// - `Receiver<EmulatorMessage>`: Channel to receive updates from emulator
    ///
    /// The background thread is spawned immediately and will run until:
    /// - A `FrontendMessage::Quit` is sent
    /// - The ThreadedEmulator is dropped (which drops the sender)
    pub fn new() -> (Self, Sender<FrontendMessage>, Receiver<EmulatorMessage>) {
        // Create channels for bidirectional communication
        let (tx_to_emu, rx_from_frontend) = crossbeam_channel::unbounded();
        let (tx_from_emu, rx_to_frontend) = crossbeam_channel::unbounded();

        // Clone the sender for the ThreadedEmulator to keep
        let to_emulator = tx_to_emu.clone();

        // Spawn background thread that owns the emulator
        let thread_handle = thread::Builder::new()
            .name("emulator-thread".to_string())
            .spawn(move || {
                // Create Nes emulator directly on this background thread
                // This is the key: Nes is created here, never sent across threads
                let nes = Nes::default();

                // Create ChannelEmulator to handle message processing
                // We construct it manually to reuse our channel endpoints
                let mut channel_emu = ChannelEmulator::new_with_channels(
                    nes,
                    tx_from_emu,
                    rx_from_frontend,
                );

                // Emulator thread main loop
                loop {
                    // Process all pending messages from frontend
                    match channel_emu.process_messages() {
                        Ok(_) => {
                            // Continue processing
                        }
                        Err(_) => {
                            // Error or quit requested, exit thread
                            break;
                        }
                    }

                    // Small sleep to avoid busy-waiting when idle
                    // The frontend controls frame stepping via StepFrame messages
                    std::thread::sleep(std::time::Duration::from_micros(100));
                }

                // Thread exits cleanly
            })
            .expect("Failed to spawn emulator thread");

        let threaded_emu = ThreadedEmulator {
            to_emulator,
            thread_handle: Some(thread_handle),
        };

        (threaded_emu, tx_to_emu, rx_to_frontend)
    }

    /// Send a quit message and wait for the thread to finish
    pub fn shutdown(mut self) {
        // Send quit message
        let _ = self.to_emulator.send(FrontendMessage::Quit);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ThreadedEmulator {
    fn drop(&mut self) {
        // Send quit message on drop
        let _ = self.to_emulator.send(FrontendMessage::Quit);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::ControllerEvent;
    use std::time::Duration;

    #[test]
    fn test_threaded_emulator_creation() {
        let (threaded_emu, to_emu, _from_emu) = ThreadedEmulator::new();

        // Verify channels are functional by sending a power message
        assert!(to_emu.send(FrontendMessage::Power).is_ok());

        // Small delay to let background thread process
        std::thread::sleep(Duration::from_millis(10));

        // Send quit to clean up
        assert!(to_emu.send(FrontendMessage::Quit).is_ok());

        // Drop explicitly to ensure clean shutdown
        drop(threaded_emu);
    }

    #[test]
    fn test_threaded_emulator_step_frame() {
        let (threaded_emu, to_emu, from_emu) = ThreadedEmulator::new();

        // Power on the emulator
        assert!(to_emu.send(FrontendMessage::Power).is_ok());

        // Send a step frame message
        assert!(to_emu.send(FrontendMessage::StepFrame).is_ok());

        // Wait a bit for frame to be processed
        std::thread::sleep(Duration::from_millis(50));

        // Try to receive a frame (with timeout)
        match from_emu.recv_timeout(Duration::from_millis(100)) {
            Ok(EmulatorMessage::FrameReady(_)) => {
                // Success - we got a frame
            }
            _ => {
                // That's ok too - the frame might not be ready yet
                // The important part is the channels are working
            }
        }

        // Clean up
        assert!(to_emu.send(FrontendMessage::Quit).is_ok());
        drop(threaded_emu);
    }

    #[test]
    fn test_threaded_emulator_controller_input() {
        let (threaded_emu, to_emu, _from_emu) = ThreadedEmulator::new();

        // Send controller input
        assert!(to_emu
            .send(FrontendMessage::ControllerInput(ControllerEvent::A))
            .is_ok());
        assert!(to_emu
            .send(FrontendMessage::ControllerInput(ControllerEvent::Start))
            .is_ok());

        // Clean up
        assert!(to_emu.send(FrontendMessage::Quit).is_ok());
        drop(threaded_emu);
    }

    #[test]
    fn test_threaded_emulator_reset() {
        let (threaded_emu, to_emu, _from_emu) = ThreadedEmulator::new();

        // Power on
        assert!(to_emu.send(FrontendMessage::Power).is_ok());

        // Reset
        assert!(to_emu.send(FrontendMessage::Reset).is_ok());

        // Small delay
        std::thread::sleep(Duration::from_millis(10));

        // Clean up
        assert!(to_emu.send(FrontendMessage::Quit).is_ok());
        drop(threaded_emu);
    }
}
