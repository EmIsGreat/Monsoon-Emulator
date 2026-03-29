//! Message handler abstractions for the egui frontend.
//!
//! This module provides a clean separation between message processing logic
//! and the main application loop. It defines traits for handling different
//! types of messages:
//!
//! - `AsyncMessageHandler`: Handles messages from async operations (file
//!   dialogs, etc.)
//! - `EmulatorMessageHandler`: Handles messages from the emulator backend
//!   (frames, debug data, etc.)
//!
//! # Architecture
//!
//! The handlers are implemented as traits on the `EguiApp` type, providing:
//! - Clean separation of concerns through separate modules
//! - Easy navigation to specific message handling code
//! - Future extensibility for new message types
//!
//! # Example
//!
//! ```ignore
//! // In the main update loop:
//! self.handle_async_messages(ctx);
//! self.handle_emulator_messages(ctx);
//! ```

pub mod async_handler;
pub mod emulator_handler;

pub use async_handler::AsyncMessageHandler;
pub use emulator_handler::EmulatorMessageHandler;
