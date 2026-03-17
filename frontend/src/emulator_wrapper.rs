/// Wrapper for emulator implementations to support both threaded and non-threaded modes.
///
/// On native platforms (non-WASM), uses `ThreadedEmulator` for better UI responsiveness.
/// On WASM, uses `ChannelEmulator` since WASM doesn't support threads.
use crossbeam_channel::{Receiver, Sender};
use monsoon_core::emulation::nes::Nes;

use crate::channel_emu::ChannelEmulator;
use crate::messages::{EmulatorMessage, FrontendMessage};

#[cfg(not(target_arch = "wasm32"))]
use crate::threaded_emu::ThreadedEmulator;

/// Emulator wrapper that abstracts over threaded and non-threaded implementations.
pub enum EmulatorWrapper {
    /// Non-threaded emulator (used on WASM, or for debugging)
    Channel(ChannelEmulator),
    /// Threaded emulator (used on native platforms for better responsiveness)
    /// Stores the ThreadedEmulator to keep the background thread alive
    #[cfg(not(target_arch = "wasm32"))]
    Threaded(ThreadedEmulator),
}

impl EmulatorWrapper {
    /// Create a new emulator wrapper.
    ///
    /// On native platforms, creates a `ThreadedEmulator` which spawns a background thread
    /// and creates the `Nes` instance on that thread (avoiding the need for `Send`).
    /// On WASM, creates a `ChannelEmulator` with the provided `console` instance.
    pub fn new(
        console: Nes,
    ) -> (
        Self,
        Sender<FrontendMessage>,
        Receiver<EmulatorMessage>,
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            // On native, use threaded emulator for better UI responsiveness
            // The ThreadedEmulator creates Nes on the background thread, so we don't use console
            let _ = console; // Explicitly mark as intentionally unused on native
            let (threaded_emu, rx_from_emu) = ThreadedEmulator::new();

            // Clone the sender for the frontend to use
            let tx_to_emu = threaded_emu.to_emulator.clone();

            (
                EmulatorWrapper::Threaded(threaded_emu),
                tx_to_emu,
                rx_from_emu,
            )
        }

        #[cfg(target_arch = "wasm32")]
        {
            // On WASM, use non-threaded emulator with the provided console
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
            EmulatorWrapper::Threaded(_) => {
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
            EmulatorWrapper::Threaded(_) => None,
        }
    }

    /// Get mutable access to the Nes instance (only available for ChannelEmulator).
    ///
    /// Returns `None` for `ThreadedEmulator` since Nes is on a different thread.
    /// When this returns `None`, you should use the message-based API instead.
    pub fn nes_mut(&mut self) -> Option<&mut Nes> {
        match self {
            EmulatorWrapper::Channel(emu) => Some(&mut emu.nes),
            #[cfg(not(target_arch = "wasm32"))]
            EmulatorWrapper::Threaded(_) => None,
        }
    }
}
