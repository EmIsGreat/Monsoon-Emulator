//! Persistent configuration and file storage utilities.
//!
//! This module provides:
//! - Directory management using the storage abstraction for cross-platform paths
//! - Generic async file read/write operations to avoid blocking the UI thread
//! - TOML-based configuration persistence for application settings
//! - Helper functions for saving files to data and cache directories

use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{fs, thread};

use crossbeam_channel::{bounded, Receiver};
use directories::ProjectDirs;
use monsoon_core::emulation::palette_util::RgbPalette;
use monsoon_core::emulation::ppu_util::EmulatorFetchable;
use monsoon_core::emulation::screen_renderer::{create_renderer, NoneRenderer, ScreenRenderer};
use serde::{Deserialize, Serialize};

use crate::frontend::egui::config::{
    AppConfig, AppSpeed, ConsoleConfig, DebugSpeed, SpeedConfig, UserConfig, ViewConfig,
};
use crate::frontend::egui::keybindings::KeybindingsConfig;
use crate::frontend::egui_frontend::get_all_renderers;
use crate::frontend::storage;
use crate::frontend::storage::{Storage, StorageKey};

/// Application identifier used for directory paths
const APP_QUALIFIER: &str = "com";
const APP_ORGANIZATION: &str = "Lightsong";
const APP_NAME: &str = "MonsoonEmulator";

/// Singleton for project directories.
///
/// This uses a `OnceLock` to lazily initialize the project directories on first access.
/// The singleton pattern is used here because:
/// 1. Directory paths are determined by the OS and don't change during runtime
/// 2. Creating `ProjectDirs` multiple times is wasteful
/// 3. All parts of the application need consistent paths for config/data/cache
///
/// Note: The directories crate returns `Option<ProjectDirs>` because on some platforms
/// (like Linux without a proper home directory) it may fail to determine paths.
static PROJECT_DIRS: OnceLock<Option<ProjectDirs>> = OnceLock::new();

/// Get the project directories (lazily initialized)
fn get_project_dirs() -> Option<&'static ProjectDirs> {
    PROJECT_DIRS
        .get_or_init(|| ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME))
        .as_ref()
}

/// Get the config directory path, creating it if necessary
pub fn get_config_dir() -> Option<PathBuf> {
    let dirs = get_project_dirs()?;
    let config_dir = dirs.config_dir();
    if !config_dir.exists() {
        fs::create_dir_all(config_dir).ok()?;
    }
    Some(config_dir.to_path_buf())
}

/// Get the data directory path, creating it if necessary
pub fn get_data_dir() -> Option<PathBuf> {
    let dirs = get_project_dirs()?;
    let data_dir = dirs.data_dir();
    if !data_dir.exists() {
        fs::create_dir_all(data_dir).ok()?;
    }
    Some(data_dir.to_path_buf())
}

/// Get the cache directory path, creating it if necessary
pub fn get_cache_dir() -> Option<PathBuf> {
    let dirs = get_project_dirs()?;
    let cache_dir = dirs.cache_dir();
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir).ok()?;
    }
    Some(cache_dir.to_path_buf())
}

/// Get the path to a file in the config directory
pub fn get_config_file_path(filename: &str) -> Option<PathBuf> {
    get_config_dir().map(|dir| dir.join(filename))
}

/// Get the path to a file in the data directory
pub fn get_data_file_path(filename: &str) -> Option<PathBuf> {
    get_data_dir().map(|dir| dir.join(filename))
}

/// Get the path to a file in the cache directory
pub fn get_cache_file_path(filename: &str) -> Option<PathBuf> {
    get_cache_dir().map(|dir| dir.join(filename))
}

// ============================================================================
// Async File Operations
// ============================================================================

/// Result type for async file operations
pub enum AsyncFileResult {
    /// File read successfully with contents
    ReadSuccess(Vec<u8>),
    /// File written successfully
    WriteSuccess,
    /// Operation failed with error message
    Error(String),
}

/// Read a file asynchronously, returning the result through a channel.
///
/// This spawns a new thread for each operation to avoid blocking the UI thread
/// when reading potentially large files. The result is returned through a
/// crossbeam channel.
///
/// Note: For frequent small file operations, consider batching requests or using
/// the synchronous `read_file_sync` if blocking is acceptable. Thread spawning
/// has overhead that may not be justified for very small files.
///
/// # Usage
/// ```ignore
/// let rx = read_file_async(path);
/// // Later, in the UI update loop:
/// if let Ok(result) = rx.try_recv() {
///     match result {
///         AsyncFileResult::ReadSuccess(data) => { /* use data */ }
///         AsyncFileResult::Error(e) => { /* handle error */ }
///         _ => {}
///     }
/// }
/// ```
pub fn read_file_async(path: PathBuf) -> Receiver<AsyncFileResult> {
    let (tx, rx) = bounded(1);
    thread::spawn(move || {
        let result = read_file_sync(&path);
        let _ = tx.send(result);
    });
    rx
}

/// Write data to a file asynchronously, returning the result through a channel.
///
/// This spawns a new thread for each operation to avoid blocking the UI thread
/// when writing files. The result is returned through a crossbeam channel.
///
/// Note: For operations that must complete before proceeding (like saving config
/// on exit), use the synchronous `save_config` function instead.
pub fn write_file_async(
    path: PathBuf,
    data: Vec<u8>,
    overwrite: bool,
) -> Receiver<AsyncFileResult> {
    let (tx, rx) = bounded(1);
    thread::spawn(move || {
        let result = write_file_sync(&path, &data, overwrite);
        let _ = tx.send(result);
    });
    rx
}

/// Read a file synchronously (internal helper)
fn read_file_sync(path: &Path) -> AsyncFileResult {
    match fs::File::open(path) {
        Ok(mut file) => {
            let mut contents = Vec::new();
            match file.read_to_end(&mut contents) {
                Ok(_) => AsyncFileResult::ReadSuccess(contents),
                Err(e) => AsyncFileResult::Error(format!("Failed to read file: {}", e)),
            }
        }
        Err(e) => AsyncFileResult::Error(format!("Failed to open file: {}", e)),
    }
}

/// Write data to a file synchronously (internal helper)
fn write_file_sync(path: &Path, data: &[u8], overwrite: bool) -> AsyncFileResult {
    // Ensure parent directory exists
    if let Some(parent) = path.parent()
        && !parent.exists()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return AsyncFileResult::Error(format!("Failed to create directory: {}", e));
    }

    if path.exists() && !overwrite {
        let copy = path
            .file_stem()
            .map(extract)
            .map(|s| s.parse::<u8>().unwrap_or(0))
            .unwrap_or(0);

        let offset = if copy == 0 { 0 } else { 2 };

        let path = append_to_filename(path, format!("_{}", copy + 1).as_str(), offset);
        write_file_sync(&path, data, overwrite)
    } else {
        match fs::File::create(path) {
            Ok(mut file) => match file.write_all(data) {
                Ok(_) => AsyncFileResult::WriteSuccess,
                Err(e) => AsyncFileResult::Error(format!("Failed to write file: {}", e)),
            },
            Err(e) => AsyncFileResult::Error(format!("Failed to create file: {}", e)),
        }
    }
}

fn extract(f: &OsStr) -> String {
    f.to_string_lossy()
        .split('_')
        .next_back()
        .unwrap_or("0")
        .to_string()
}

/// Append a suffix to a filename before the extension
fn append_to_filename(path: &Path, suffix: &str, strip_chars: usize) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().map(|e| e.to_string_lossy().to_string());

    // Strip the specified number of characters from the end of the stem
    let trimmed_stem = if strip_chars > 0 && stem.len() >= strip_chars {
        &stem[..stem.len() - strip_chars]
    } else {
        &stem
    };

    let new_filename = match ext {
        Some(e) => format!("{}{}.{}", trimmed_stem, suffix, e),
        None => format!("{}{}", trimmed_stem, suffix),
    };

    path.with_file_name(new_filename)
}

/// Save data to the data directory asynchronously
pub fn save_to_data_dir(
    filename: &str,
    data: Vec<u8>,
    overwrite: bool,
) -> Option<Receiver<AsyncFileResult>> {
    let path = get_data_file_path(filename)?;
    Some(write_file_async(path, data, overwrite))
}

/// Save data to the cache directory asynchronously
pub fn save_to_cache_dir(
    filename: &str,
    data: Vec<u8>,
    overwrite: bool,
) -> Option<Receiver<AsyncFileResult>> {
    let path = get_cache_file_path(filename)?;
    Some(write_file_async(path, data, overwrite))
}

/// Read data from the data directory asynchronously
pub fn read_from_data_dir(filename: &str) -> Option<Receiver<AsyncFileResult>> {
    let path = get_data_file_path(filename)?;
    if path.exists() {
        Some(read_file_async(path))
    } else {
        None
    }
}

/// Read data from the cache directory asynchronously
pub fn read_from_cache_dir(filename: &str) -> Option<Receiver<AsyncFileResult>> {
    let path = get_cache_file_path(filename)?;
    if path.exists() {
        Some(read_file_async(path))
    } else {
        None
    }
}

// ============================================================================
// Configuration Persistence (TOML format)
// ============================================================================

/// Persistent configuration structure that can be serialized to TOML.
///
/// Uses `RendererKind` for runtime-switchable renderer persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistentConfig {
    pub user_config: PersistentUserConfig,
    pub view_config: PersistentViewConfig,
    pub speed_config: PersistentSpeedConfig,
    pub console_config: PersistentConsoleConfig,
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

impl From<&AppConfig> for PersistentConfig {
    fn from(value: &AppConfig) -> Self {
        Self {
            user_config: (&value.user_config).into(),
            view_config: (&value.view_config).into(),
            speed_config: (&value.speed_config).into(),
            console_config: (&value.console_config).into(),
            keybindings: value.keybindings.clone(),
        }
    }
}

impl From<&PersistentConfig> for AppConfig {
    fn from(value: &PersistentConfig) -> Self {
        Self {
            view_config: (&value.view_config).into(),
            speed_config: (&value.speed_config).into(),
            user_config: (&value.user_config).into(),
            console_config: (&value.console_config).into(),
            pending_dialogs: Default::default(),
            keybindings: value.keybindings.clone(),
        }
    }
}

/// Persistent View configuration with RendererKind for runtime switching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentViewConfig {
    pub show_palette: bool,
    pub show_pattern_table: bool,
    pub show_nametable: bool,
    pub required_debug_fetches: HashSet<PersistentEmulatorFetchable>,
    pub debug_active_palette: usize,
    /// The serialized renderer state. When present, the renderer is restored from this.
    /// When absent (e.g., first run), a default renderer is created.
    #[serde(default)]
    pub renderer: String,
}

impl Default for PersistentViewConfig {
    fn default() -> Self {
        Self {
            show_palette: false,
            show_pattern_table: false,
            show_nametable: false,
            required_debug_fetches: HashSet::new(),
            debug_active_palette: 0,
            renderer: NoneRenderer::new().get_id().to_string(),
        }
    }
}

impl From<&ViewConfig> for PersistentViewConfig {
    fn from(config: &ViewConfig) -> Self {
        Self {
            show_palette: config.show_palette,
            show_pattern_table: config.show_pattern_table,
            show_nametable: config.show_nametable,
            required_debug_fetches: config
                .required_debug_fetches
                .iter()
                .map(|f| f.into())
                .collect(),
            debug_active_palette: config.debug_active_palette,
            renderer: config.renderer.get_id().to_string(),
        }
    }
}

impl From<&PersistentViewConfig> for ViewConfig {
    fn from(config: &PersistentViewConfig) -> Self {
        // If renderer was persisted, use it; otherwise create a default
        let renderer = create_renderer(Some(config.renderer.as_str()), get_all_renderers());

        Self {
            debug_active_palette: config.debug_active_palette,
            palette_rgb_data: RgbPalette::default(),
            show_nametable: config.show_nametable,
            show_palette: config.show_palette,
            show_pattern_table: config.show_pattern_table,
            required_debug_fetches: Default::default(),
            renderer,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Hash)]
pub enum PersistentEmulatorFetchable {
    Palettes,
    Tiles,
    Nametables,
    Sprites,
}

impl From<&EmulatorFetchable> for PersistentEmulatorFetchable {
    fn from(fetchable: &EmulatorFetchable) -> Self {
        match fetchable {
            EmulatorFetchable::Palettes(_) => PersistentEmulatorFetchable::Palettes,
            EmulatorFetchable::Tiles(_) => PersistentEmulatorFetchable::Tiles,
            EmulatorFetchable::Nametables(_) => PersistentEmulatorFetchable::Nametables,
            EmulatorFetchable::Sprites(_) => PersistentEmulatorFetchable::Sprites,
        }
    }
}

impl From<PersistentEmulatorFetchable> for EmulatorFetchable {
    fn from(fetchable: PersistentEmulatorFetchable) -> Self {
        match fetchable {
            PersistentEmulatorFetchable::Palettes => EmulatorFetchable::Palettes(None),
            PersistentEmulatorFetchable::Tiles => EmulatorFetchable::Tiles(None),
            PersistentEmulatorFetchable::Nametables => EmulatorFetchable::Nametables(None),
            PersistentEmulatorFetchable::Sprites => EmulatorFetchable::Sprites(None),
        }
    }
}

/// Persistent user configuration - stores display names instead of paths for WASM compatibility
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistentUserConfig {
    /// Last loaded palette filename (display only)
    pub previous_palette_name: Option<String>,
    /// Last loaded palette directory (for file picker initial directory)
    pub previous_palette_dir: Option<StorageKey>,
    /// Last loaded ROM filename (display only)
    pub previous_rom_name: Option<String>,
    /// Last loaded ROM directory (for file picker initial directory)
    pub previous_rom_dir: Option<StorageKey>,
    /// Last loaded savestate filename (display only)
    pub previous_savestate_name: Option<String>,
    /// Last loaded savestate directory (for file picker initial directory)
    pub previous_savestate_dir: Option<StorageKey>,
    /// Last saved palette directory (for file picker initial directory)
    #[serde(default)]
    pub previous_palette_save_dir: Option<StorageKey>,
    /// Last saved savestate directory (for file picker initial directory)
    #[serde(default)]
    pub previous_savestate_save_dir: Option<StorageKey>,
    pub pattern_edit_color: u8,
}

impl From<&UserConfig> for PersistentUserConfig {
    fn from(config: &UserConfig) -> Self {
        Self {
            previous_palette_name: config.previous_palette_name.clone(),
            previous_palette_dir: config.previous_palette_dir.clone(),
            previous_rom_name: config.previous_rom_name.clone(),
            previous_rom_dir: config.previous_rom_dir.clone(),
            previous_savestate_name: config.previous_savestate_name.clone(),
            previous_savestate_dir: config.previous_savestate_dir.clone(),
            previous_palette_save_dir: config.previous_palette_save_dir.clone(),
            previous_savestate_save_dir: config.previous_savestate_save_dir.clone(),
            pattern_edit_color: config.pattern_edit_color,
        }
    }
}

impl From<&PersistentUserConfig> for UserConfig {
    fn from(config: &PersistentUserConfig) -> Self {
        Self {
            previous_palette_name: config.previous_palette_name.clone(),
            previous_palette_dir: config.previous_palette_dir.clone(),
            previous_rom_name: config.previous_rom_name.clone(),
            previous_rom_dir: config.previous_rom_dir.clone(),
            previous_savestate_name: config.previous_savestate_name.clone(),
            previous_savestate_dir: config.previous_savestate_dir.clone(),
            previous_palette_save_dir: config.previous_palette_save_dir.clone(),
            previous_savestate_save_dir: config.previous_savestate_save_dir.clone(),
            pattern_edit_color: config.pattern_edit_color,
            loaded_rom: None,
        }
    }
}

/// Persistent speed configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentSpeedConfig {
    pub app_speed: PersistentAppSpeed,
    pub debug_speed: PersistentDebugSpeed,
    pub custom_speed: u16,
    pub debug_custom_speed: u16,
}

impl Default for PersistentSpeedConfig {
    fn default() -> Self {
        Self {
            app_speed: PersistentAppSpeed::DefaultSpeed,
            debug_speed: PersistentDebugSpeed::Default,
            custom_speed: 100,
            debug_custom_speed: 10,
        }
    }
}

/// Persistent app speed enum (serializable version)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum PersistentAppSpeed {
    #[default]
    DefaultSpeed,
    Uncapped,
    Custom,
}

impl From<AppSpeed> for PersistentAppSpeed {
    fn from(speed: AppSpeed) -> Self {
        match speed {
            AppSpeed::DefaultSpeed => PersistentAppSpeed::DefaultSpeed,
            AppSpeed::Uncapped => PersistentAppSpeed::Uncapped,
            AppSpeed::Custom => PersistentAppSpeed::Custom,
        }
    }
}

impl From<PersistentAppSpeed> for AppSpeed {
    fn from(speed: PersistentAppSpeed) -> Self {
        match speed {
            PersistentAppSpeed::DefaultSpeed => AppSpeed::DefaultSpeed,
            PersistentAppSpeed::Uncapped => AppSpeed::Uncapped,
            PersistentAppSpeed::Custom => AppSpeed::Custom,
        }
    }
}

/// Persistent debug speed enum (serializable version)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum PersistentDebugSpeed {
    #[default]
    Default,
    InStep,
    Custom,
}

impl From<DebugSpeed> for PersistentDebugSpeed {
    fn from(speed: DebugSpeed) -> Self {
        match speed {
            DebugSpeed::DefaultSpeed => PersistentDebugSpeed::Default,
            DebugSpeed::InStep => PersistentDebugSpeed::InStep,
            DebugSpeed::Custom => PersistentDebugSpeed::Custom,
        }
    }
}

impl From<PersistentDebugSpeed> for DebugSpeed {
    fn from(speed: PersistentDebugSpeed) -> Self {
        match speed {
            PersistentDebugSpeed::Default => DebugSpeed::DefaultSpeed,
            PersistentDebugSpeed::InStep => DebugSpeed::InStep,
            PersistentDebugSpeed::Custom => DebugSpeed::Custom,
        }
    }
}

impl From<&SpeedConfig> for PersistentSpeedConfig {
    fn from(config: &SpeedConfig) -> Self {
        Self {
            app_speed: config.app_speed.into(),
            debug_speed: config.debug_speed.into(),
            custom_speed: config.custom_speed,
            debug_custom_speed: config.debug_custom_speed,
        }
    }
}

impl From<&PersistentSpeedConfig> for SpeedConfig {
    fn from(config: &PersistentSpeedConfig) -> Self {
        Self {
            app_speed: config.app_speed.into(),
            debug_speed: config.debug_speed.into(),
            is_paused: false, // Transient state - don't persist
            custom_speed: config.custom_speed,
            debug_custom_speed: config.debug_custom_speed,
        }
    }
}

/// Persistent console configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentConsoleConfig {
    pub is_powered: bool,
}

impl Default for PersistentConsoleConfig {
    fn default() -> Self {
        Self {
            is_powered: true,
        }
    }
}

impl From<&ConsoleConfig> for PersistentConsoleConfig {
    fn from(config: &ConsoleConfig) -> Self {
        Self {
            is_powered: config.is_powered,
        }
    }
}

impl From<&PersistentConsoleConfig> for ConsoleConfig {
    fn from(config: &PersistentConsoleConfig) -> Self {
        Self {
            is_powered: config.is_powered,
        }
    }
}

/// Load configuration from the config file asynchronously.
///
/// Returns `None` if:
/// - The config file doesn't exist (first run)
/// - The config file cannot be read or parsed
///
/// Parsing errors are logged to stderr but not treated as fatal,
/// allowing the application to start with default config if the
/// config file is malformed.
pub async fn load_config() -> Option<PersistentConfig> {
    let key = storage::config_key();
    let storage_impl = storage::get_storage();

    // Check if config exists using storage
    match storage_impl.exists(&key).await {
        Ok(false) => return None,
        Err(e) => {
            eprintln!("Failed to check if config exists: {}", e);
            return None;
        }
        Ok(true) => {}
    }

    let contents = match storage_impl.get(&key).await {
        Ok(data) => match String::from_utf8(data) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Config file is not valid UTF-8: {}", e);
                return None;
            }
        },
        Err(e) => {
            eprintln!("Failed to read config file: {}", e);
            return None;
        }
    };

    match toml::from_str(&contents) {
        Ok(config) => Some(config),
        Err(e) => {
            eprintln!("Failed to parse config file (using defaults): {}", e);
            None
        }
    }
}

/// Save configuration to the config file asynchronously
pub async fn save_config(config: &PersistentConfig) -> Result<(), String> {
    let key = storage::config_key();
    let storage_impl = storage::get_storage();

    let toml_string =
        toml::to_string_pretty(config).map_err(|e| format!("Failed to serialize config: {}", e))?;

    storage_impl
        .set(&key, toml_string.as_bytes().to_vec())
        .await
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}

// ============================================================================
// Egui Storage Path
// ============================================================================

/// Get the path for egui's persistence storage
///
/// This is used to enable egui's built-in persistence for window layout,
/// theme, and other UI state.
pub fn get_egui_storage_path() -> Option<PathBuf> {
    get_config_dir().map(|dir| dir.join("egui_state"))
}
