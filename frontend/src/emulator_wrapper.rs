/// Wrapper for emulator implementations to support both threaded and non-threaded modes.
///
/// On native platforms (non-WASM), uses `ThreadedEmulator` for better UI responsiveness.
/// On WASM, uses `ChannelEmulator` since WASM doesn't support threads.
use crossbeam_channel::{Receiver, Sender};
use monsoon_core::emulation::nes::Nes;

use crate::channel_emu::ChannelEmulator;
use crate::messages::{EmulatorMessage, FrontendMessage};

/// Emulator wrapper that abstracts over threaded and non-threaded implementations.
pub enum EmulatorWrapper {
    /// Non-threaded emulator (used on WASM, or for debugging)
    Channel(ChannelEmulator),
    /// Threaded emulator (used on native platforms for better responsiveness)
    /// We don't need to store anything here since ThreadedEmulator is dropped when the struct drops
    #[cfg(not(target_arch = "wasm32"))]
    Threaded,
}

impl EmulatorWrapper {
    /// Create a new emulator wrapper.
    ///
    /// On native platforms, creates a `ThreadedEmulator`.
    /// On WASM, creates a `ChannelEmulator`.
    pub fn new(
        console: Nes,
    ) -> (
        Self,
        Sender<FrontendMessage>,
        Receiver<EmulatorMessage>,
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use crate::threaded_emu::ThreadedEmulator;

            // On native, use threaded emulator for better UI responsiveness
            let (threaded_emu, rx_from_emu) = ThreadedEmulator::new();

            // Create a channel for the frontend to send messages
            // We'll forward these to the threaded emulator
            let (tx_to_emu, rx_to_emu) = crossbeam_channel::unbounded();

            // Spawn a forwarding thread
            // ThreadedEmulator manages its own thread lifetime via Drop
            std::thread::spawn(move || {
                while let Ok(msg) = rx_to_emu.recv() {
                    // Forward messages to the threaded emulator
                    if threaded_emu.send(msg).is_err() {
                        break;
                    }
                }
                // threaded_emu will be dropped here, which sends Quit and joins the thread
            });

            (
                EmulatorWrapper::Threaded,
                tx_to_emu,
                rx_from_emu,
            )
        }

        #[cfg(target_arch = "wasm32")]
        {
            // On WASM, use non-threaded emulator
            let (emu, tx_to_emu, rx_from_emu) = ChannelEmulator::new(console);
            (EmulatorWrapper::Channel(emu), tx_to_emu, rx_from_emu)
        }
    }

    /// Process pending messages from the frontend.
    ///
    /// For `ChannelEmulator`, this calls `process_messages()`.
    /// For `ThreadedEmulator`, this is a no-op (messages are processed on background thread).
    pub fn process_messages(&mut self) -> Result<(), String> {
        match self {
            EmulatorWrapper::Channel(emu) => emu.process_messages(),
            #[cfg(not(target_arch = "wasm32"))]
            EmulatorWrapper::Threaded => {
                // Threaded emulator processes messages on background thread
                Ok(())
            }
        }
    }

    /// Get access to the Nes instance (only available for ChannelEmulator).
    ///
    /// Returns `None` for `ThreadedEmulator` since Nes is on a different thread.
    /// When this returns `None`, you should use the message-based API instead.
    pub fn nes(&self) -> Option<&Nes> {
        match self {
            EmulatorWrapper::Channel(emu) => Some(&emu.nes),
            #[cfg(not(target_arch = "wasm32"))]
            EmulatorWrapper::Threaded => None,
        }
    }
}
