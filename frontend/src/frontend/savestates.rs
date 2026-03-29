use crate::frontend::messages::{LoadedRom, SavestateLoadContext};
use crate::frontend::storage::StorageKey;

/// A single save entry for display in the save browser
#[derive(Clone, Hash)]
pub struct SaveEntry {
    /// Storage key to read this save
    pub key: StorageKey,
    /// Display name (extracted from filename)
    pub display_name: String,
    /// Timestamp string extracted from the filename
    pub timestamp: String,
    /// Whether this is a quicksave or autosave
    pub save_type: SaveEntryType,
}

/// Type of save entry
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveEntryType {
    Quicksave,
    Autosave,
}

impl std::fmt::Display for SaveEntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveEntryType::Quicksave => write!(f, "Quicksave"),
            SaveEntryType::Autosave => write!(f, "Autosave"),
        }
    }
}

/// State for the matching ROM dialog
#[derive(Clone)]
pub struct MatchingRomDialogState {
    pub context: Box<SavestateLoadContext>,
    /// The matching ROM data that was found
    pub matching_rom: LoadedRom,
}

/// State for the checksum mismatch warning dialog
#[derive(Clone)]
pub struct ChecksumMismatchDialogState {
    pub context: Box<SavestateLoadContext>,
    /// The selected ROM data (with mismatched checksum)
    pub selected_rom: LoadedRom,
}

/// State for the ROM selection dialog
#[derive(Clone)]
pub struct RomSelectionDialogState {
    pub context: Box<SavestateLoadContext>,
}

/// State for a generic error dialog
#[derive(Clone)]
pub struct ErrorDialogState {
    pub title: String,
    pub message: String,
}

/// State for the save browser dialog
#[derive(Clone)]
pub struct SaveBrowserState {
    /// All save entries loaded from storage
    pub entries: Vec<SaveEntry>,
    /// The ROM display name these saves belong to
    pub game_name: String,
    /// Whether entries are still being loaded
    pub loading: bool,
    /// Filter: show quicksaves
    pub show_quicksaves: bool,
    /// Filter: show autosaves
    pub show_autosaves: bool,
}
