use crossbeam_channel::{Receiver, Sender};

#[cfg(target_arch = "wasm32")]
use monsoon_core::emulation::nes::Nes;

use crate::channel_emu::ChannelEmulator;
#[cfg(not(target_arch = "wasm32"))]
use crate::threaded_emu::ThreadedEmulator;
use crate::messages::{EmulatorMessage, FrontendMessage};

/// Unified emulator wrapper that abstracts over threaded and non-threaded implementations.
///
/// On native platforms, uses ThreadedEmulator for true parallel execution.
/// On WASM, uses ChannelEmulator since WASM doesn't support threads.
///
/// This provides a consistent API regardless of the underlying implementation.
///
/// # Architecture
///
/// Native (with threading):
/// ```text
/// EguiApp → EmulatorWrapper::Threaded → Background Thread
///                                            ↓
///                                       ChannelEmulator
///                                            ↓
///                                          Nes (with Rc<RefCell<Ppu>>)
/// ```
///
/// WASM (no threading):
/// ```text
/// EguiApp → EmulatorWrapper::Channel → ChannelEmulator → Nes
/// ```
///
/// The key insight is that on native, the Nes is created on the background
/// thread, so it never needs to be Send. This avoids the performance cost of
/// Arc<Mutex<>> while still enabling true parallel execution.
pub enum EmulatorWrapper {
    /// Threaded emulator (native only) - emulator runs on background thread
    #[cfg(not(target_arch = "wasm32"))]
    Threaded(ThreadedEmulator),

    /// Non-threaded channel-based emulator (WASM or fallback)
    /// Boxed to reduce enum size disparity
    Channel(Box<ChannelEmulator>),
}

impl EmulatorWrapper {
    /// Create a new emulator wrapper.
    ///
    /// On native platforms, spawns a background thread with the emulator.
    /// On WASM, creates a single-threaded ChannelEmulator.
    ///
    /// Returns the wrapper and channel endpoints for communication.
    pub fn new() -> (Self, Sender<FrontendMessage>, Receiver<EmulatorMessage>) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (threaded_emu, to_emu, from_emu) = ThreadedEmulator::new();
            (Self::Threaded(threaded_emu), to_emu, from_emu)
        }

        #[cfg(target_arch = "wasm32")]
        {
            let nes = Nes::default();
            let (channel_emu, to_emu, from_emu) = ChannelEmulator::new(nes);
            (Self::Channel(Box::new(channel_emu)), to_emu, from_emu)
        }
    }

    /// Execute a single frame of emulation.
    ///
    /// On threaded implementation, this sends a StepFrame message.
    /// On non-threaded implementation, this executes synchronously.
    pub fn execute_frame(&mut self) -> Result<(), String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(_) => {
                // For threaded emulator, we don't call execute_frame directly
                // The frontend sends StepFrame messages through the channel
                // This method is kept for API compatibility but should not be used
                // with threaded emulator
                Ok(())
            }
            Self::Channel(emu) => emu.execute_frame(),
        }
    }

    /// Process messages from the frontend.
    ///
    /// For non-threaded emulator, this processes messages synchronously.
    /// For threaded emulator, messages are already being processed on the background thread.
    pub fn process_messages(&mut self) -> Result<(), String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Threaded(_) => {
                // Messages are already being processed by the background thread
                // This is a no-op for threaded emulator
                Ok(())
            }
            Self::Channel(emu) => emu.process_messages(),
        }
    }

    /// Get direct access to the Nes instance (WASM only).
    ///
    /// This is only available on WASM where we have synchronous access.
    /// On native with threading, the Nes lives on a different thread.
    #[cfg(target_arch = "wasm32")]
    pub fn nes(&self) -> &Nes {
        match self {
            Self::Channel(emu) => &emu.nes,
        }
    }

    /// Get mutable access to the Nes instance (WASM only).
    ///
    /// This is only available on WASM where we have synchronous access.
    /// On native with threading, the Nes lives on a different thread.
    #[cfg(target_arch = "wasm32")]
    pub fn nes_mut(&mut self) -> &mut Nes {
        match self {
            Self::Channel(emu) => &mut emu.nes,
        }
    }
}

impl Default for EmulatorWrapper {
    fn default() -> Self {
        Self::new().0
    }
}
