/// Egui-based frontend for the NES emulator using eframe.
///
/// This frontend provides a modern, multi-window debugging interface with:
/// - Real-time emulator output display with proper texture rendering
/// - Pattern table viewer
/// - Nametable viewer
/// - FPS counter and status display
/// - Extensible window system for future debug features
///
/// # Architecture
///
/// The frontend uses message-based communication with the emulator:
/// - Sends `FrontendMessage` for commands (pause, resume, quit, etc.)
/// - Receives `EmulatorMessage` for updates (frame ready, stopped, etc.)
///
/// # Threading
///
/// Currently runs in a single thread with the emulator for simplicity.
/// The architecture supports future multi-threading once the emulator
/// core is refactored to use `Arc<Mutex<>>` instead of `Rc<RefCell<>>`.
///
/// # Module Structure
///
/// - `config`: Configuration types for app state and settings
/// - `fps_counter`: FPS tracking utility
/// - `input`: Input handling logic
/// - `keybindings`: Configurable keybindings
/// - `message_handlers`: Async and emulator message processing
/// - `textures`: Texture storage and management
/// - `ui`: UI rendering modules for different panels and windows
pub mod config;
pub mod fps_counter;
pub mod input;
pub mod keybindings;
pub mod message_handlers;
pub mod textures;
pub mod tiles;
pub mod ui;
pub mod wgpu_renderer;
