use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use egui::{Key, Modifiers};
use monsoon_core::emulation::palette_util::RgbPalette;
use monsoon_core::emulation::ppu_util::EmulatorFetchable;
use monsoon_core::emulation::rom::RomFile;
use monsoon_core::emulation::screen_renderer::{create_renderer, ScreenRenderer};
use serde::{Deserialize, Serialize};

use crate::frontend::egui::keybindings::{
    BindVariant, Binding, HotkeyBinding, ModifierKey, OnKeyAction,
};
use crate::frontend::egui_frontend::get_all_renderers;
use crate::frontend::messages::LoadedRom;
use crate::frontend::savestates::{
    ChecksumMismatchDialogState, ErrorDialogState, MatchingRomDialogState, RomSelectionDialogState,
    SaveBrowserState,
};
use crate::frontend::storage::StorageKey;

/// View configuration for the emulator frontend.
///
/// Contains settings related to rendering and debug viewers.
#[derive(Debug)]
pub struct ViewConfig {
    pub show_palette: bool,
    pub show_pattern_table: bool,
    pub show_nametable: bool,
    pub required_debug_fetches: HashSet<EmulatorFetchable>,
    /// The renderer instance used for converting palette indices to RGB colors.
    /// This can be changed at runtime by replacing with a different
    /// `RendererKind` variant.
    pub renderer: Box<dyn ScreenRenderer>,
    /// The RGB palette data used for rendering (kept for debug viewers like
    /// pattern tables).
    pub palette_rgb_data: RgbPalette,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            show_palette: false,
            show_pattern_table: false,
            show_nametable: false,
            required_debug_fetches: HashSet::new(),
            renderer: create_renderer(Some("PaletteLookup"), get_all_renderers()),
            palette_rgb_data: RgbPalette::default(),
        }
    }
}

/// Main application configuration.
///
/// Note: `Eq` and `PartialEq` are not derived because `PendingDialogs` contains
/// `SavestateLoadContext` which includes `SaveState`, which is not trivially
/// comparable.
#[derive(Default)]
pub struct AppConfig {
    pub view_config: ViewConfig,
    pub speed_config: SpeedConfig,
    pub user_config: UserConfig,
    pub console_config: ConsoleConfig,
    pub pending_dialogs: PendingDialogs,
    pub keybindings: KeybindingsConfig,
}

/// Pending dialog states for multi-step operations
#[derive(Default, Clone)]
pub struct PendingDialogs {
    /// Dialog to ask user if they want to use a matching ROM found in the
    /// directory
    pub matching_rom_dialog: Option<MatchingRomDialogState>,
    /// Dialog to ask user what to do when ROM checksum doesn't match
    pub checksum_mismatch_dialog: Option<ChecksumMismatchDialogState>,
    /// Dialog to ask user to select a ROM file (shows expected filename)
    pub rom_selection_dialog: Option<RomSelectionDialogState>,
    /// Generic error dialog for displaying error messages
    pub error_dialog: Option<ErrorDialogState>,
    /// Save browser dialog for listing and loading internal saves
    pub save_browser: Option<SaveBrowserState>,
}

/// User configuration - stores display names and directory hints for WASM
/// compatibility
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct UserConfig {
    /// Last loaded palette filename (display only, for persistence)
    pub previous_palette_name: Option<String>,
    /// Last loaded ROM filename (display only, for persistence)
    pub previous_rom_name: Option<String>,
    /// Last loaded savestate filename (display only, for persistence)
    pub previous_savestate_name: Option<String>,
    /// Last loaded ROM directory (for file picker initial directory)
    pub previous_rom_load_dir: Option<StorageKey>,
    /// Last saved savestate directory (for file picker initial directory)
    pub previous_savestate_save_dir: Option<StorageKey>,
    /// Last loaded savestate directory (for file picker initial directory)
    pub previous_savestate_load_dir: Option<StorageKey>,
    /// Last saved palette directory (for file picker initial directory)
    pub previous_palette_save_dir: Option<StorageKey>,
    /// Last loaded palette directory (for file picker initial directory)
    pub previous_palette_load_dir: Option<StorageKey>,
    pub debug_active_palette: usize,
    pub pattern_edit_color: u8,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConsoleConfig {
    pub is_powered: bool,
    pub loaded_rom: Option<(RomFile, LoadedRom)>,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            is_powered: true,
            loaded_rom: None,
        }
    }
}

/// Emulation speed mode
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum AppSpeed {
    #[default]
    DefaultSpeed,
    Uncapped,
    Custom,
}

impl AppSpeed {
    pub fn get_fps(&self, speed_config: &SpeedConfig) -> f32 {
        match self {
            AppSpeed::DefaultSpeed => 60.0988,
            AppSpeed::Uncapped => f32::MAX,
            AppSpeed::Custom => 60.0988 * (speed_config.custom_speed as f32 / 100.0),
        }
    }
}

/// Debug viewer speed mode
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum DebugSpeed {
    #[default]
    DefaultSpeed,
    InStep,
    Custom,
}

impl DebugSpeed {
    pub fn get_fps(&self, speed_config: &SpeedConfig) -> f32 {
        match self {
            DebugSpeed::DefaultSpeed => 10.0,
            DebugSpeed::InStep => speed_config.app_speed.get_fps(speed_config),
            DebugSpeed::Custom => {
                if speed_config.debug_custom_speed == 0 {
                    return 0.0;
                }

                if speed_config.app_speed == AppSpeed::Uncapped {
                    return 10.0;
                }

                ((speed_config.debug_custom_speed as f64 / 100.0)
                    * speed_config.app_speed.get_fps(speed_config) as f64)
                    .max(1.0) as f32
            }
        }
    }
}

/// Speed-related configuration
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SpeedConfig {
    pub app_speed: AppSpeed,
    pub debug_speed: DebugSpeed,
    pub is_paused: bool,
    pub custom_speed: u16,
    pub debug_custom_speed: u16,
}

impl Default for SpeedConfig {
    fn default() -> Self {
        Self {
            app_speed: Default::default(),
            debug_speed: Default::default(),
            is_paused: false,
            custom_speed: 100,
            debug_custom_speed: 10,
        }
    }
}

/// All keybindings for the emulator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeybindingsConfig {
    pub keybindings: HashMap<OnKeyAction, Binding>,
}

impl KeybindingsConfig {
    /// Reset all keybindings to defaults.
    pub fn reset_to_defaults(&mut self) { *self = Self::default(); }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        let mut bindings = Vec::new();

        bindings.push(Binding::key(Key::W, OnKeyAction::ControllerUp));
        bindings.push(Binding::key(Key::S, OnKeyAction::ControllerDown));
        bindings.push(Binding::key(Key::A, OnKeyAction::ControllerLeft));
        bindings.push(Binding::key(Key::D, OnKeyAction::ControllerRight));
        bindings.push(Binding::key(Key::Space, OnKeyAction::ControllerAButton));
        bindings.push(Binding::new(
            BindVariant::ModifierKey(ModifierKey::Shift),
            Modifiers::NONE,
            OnKeyAction::ControllerBButton,
        ));
        bindings.push(Binding::key(Key::Enter, OnKeyAction::ControllerStartButton));
        bindings.push(Binding::key(Key::Tab, OnKeyAction::ControllerSelectButton));

        bindings.push(Binding::key(Key::Comma, OnKeyAction::PauseEmulator));
        bindings.push(Binding::key(Key::Period, OnKeyAction::StepFrame));
        bindings.push(Binding::with_modifiers(
            Key::Period,
            Modifiers::CTRL,
            OnKeyAction::StepScanline,
        ));
        bindings.push(Binding::key(Key::Slash, OnKeyAction::StepMasterCycle));
        bindings.push(Binding::with_modifiers(
            Key::Slash,
            Modifiers::ALT,
            OnKeyAction::StepCpuCycle,
        ));
        bindings.push(Binding::with_modifiers(
            Key::Slash,
            Modifiers::SHIFT,
            OnKeyAction::StepPpuCycle,
        ));
        bindings.push(Binding::key(Key::R, OnKeyAction::Reset));
        bindings.push(Binding::key(Key::F5, OnKeyAction::Quicksave));
        bindings.push(Binding::key(Key::F8, OnKeyAction::Quickload));
        bindings.push(Binding::key(Key::N, OnKeyAction::ChangeDebugPalette));

        let mut map = HashMap::new();

        for bind in bindings {
            map.insert(bind.logical_bind, bind.clone());
        }

        KeybindingsConfig {
            keybindings: map,
        }
    }
}
