//! UI components for the emulator frontend

mod emulator_output;
mod keybindings_pane;
mod menu_bar;
mod nametable_pane;
mod options_pane;
mod palettes;
mod pattern_table;
mod pattern_table_pane;
pub mod save_browser;
pub mod savestate_dialogs;
mod status_bar;
pub mod widgets;

pub use emulator_output::render_emulator_output;
pub use keybindings_pane::render_keybindings;
pub use menu_bar::add_menu_bar;
pub use nametable_pane::render_nametable;
pub use options_pane::render_options;
pub use palettes::render_palettes;
pub use pattern_table::draw_pattern_table;
pub use pattern_table_pane::render_pattern_table;
pub use save_browser::render_save_browser;
pub use savestate_dialogs::render_dialogs as render_savestate_dialogs;
pub use status_bar::add_status_bar;
