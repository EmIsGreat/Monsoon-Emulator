use monsoon_core::emulation::palette_util::RgbPalette;
use monsoon_core::emulation::savestate::SaveState;
use serde::{Deserialize, Serialize};

use crate::frontend::savestates::SaveEntry;
use crate::frontend::storage::StorageKey;
use crate::frontend::util::{FileType, SavestateLoadError};
use crate::messages::ControllerEvent;

/// Visual/frontend-only events that are processed synchronously via a deque.
///
/// These events only affect the frontend UI state and don't communicate with
/// the emulator. They are pushed to a deque and processed on each update call,
/// avoiding channel overhead for internal frontend operations.
pub enum FrontendEvent {
    /// Change the window title
    ChangeWindowTitle(String),
    /// Refresh palette textures
    RefreshPalette,
}

/// Represents file data loaded from a file picker.
/// Contains the raw bytes, the filename, and optionally the directory.
#[derive(Clone)]
pub struct LoadedFile {
    /// Raw file data bytes
    pub data: Vec<u8>,
    /// Filename (without path, e.g. "game.nes")
    pub name: String,
    /// Directory path (may be None on WASM)
    pub directory: Option<String>,
}

/// Represents ROM data loaded from a file picker.
/// Contains the raw bytes, the filename, and optionally the directory.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct LoadedRom {
    /// Raw ROM data bytes
    pub data: Vec<u8>,
    /// ROM filename (without path, e.g. "game.nes")
    pub name: String,
    /// Directory path (may be None on WASM)
    pub directory: StorageKey,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LoadedPalette {
    pub palette: RgbPalette,
    pub directory: StorageKey,
}

#[derive(Clone)]
/// Messages for async/deferred frontend operations.
///
/// These messages are processed by EguiApp and allow UI components to request
/// operations without directly sending FrontendMessages to the emulator.
/// This consolidates all emulator communication logic in one place.
pub enum AsyncFrontendMessage {
    /// Palette file was loaded asynchronously - includes the parsed palette
    /// data and directory
    PaletteLoaded(LoadedPalette),
    /// User has selected a savestate file, now need to verify/select ROM
    SavestateLoaded(Box<SavestateLoadContext>),
    /// Show dialog asking if user wants to load the found matching ROM (native
    /// only - we found a ROM in same dir)
    ShowMatchingRomDialog(Box<SavestateLoadContext>, LoadedRom),
    /// User chose to use the matching ROM that was found
    UseMatchingRom(Box<SavestateLoadContext>, LoadedRom),
    /// User chose to manually select a ROM for the savestate
    ManuallySelectRom(Box<SavestateLoadContext>),
    /// User selected a ROM file for the savestate - contains ROM data
    RomSelectedForSavestate(Box<SavestateLoadContext>, LoadedRom),
    /// Show checksum mismatch warning dialog
    ShowChecksumMismatchDialog(Box<SavestateLoadContext>, LoadedRom),
    /// User chose to try loading with mismatched checksum anyway
    LoadSavestateAnyway(Box<SavestateLoadContext>, LoadedRom),
    /// User chose to select another ROM after checksum mismatch
    SelectAnotherRom(Box<SavestateLoadContext>),
    /// An error occurred while loading the savestate
    SavestateLoadFailed(SavestateLoadError),
    /// An error occurred while verifying the ROM
    RomVerificationFailed(Box<SavestateLoadContext>, SavestateLoadError),
    /// File save completed (success or error message, with directory and file
    /// type for persistence)
    FileSaveCompleted {
        error: Option<String>,
        directory: Option<StorageKey>,
        file_type: FileType,
    },
    Quickload,
    Quicksave,
    /// Load a ROM - contains ROM data if provided, None triggers file picker
    LoadRom(Option<LoadedRom>),
    StartLoadRom,
    /// Open the save browser dialog (triggers async listing of saves)
    OpenSaveBrowser,
    /// Save browser has finished loading entries
    SaveBrowserLoaded(Vec<SaveEntry>),
    /// Load a specific save from the browser by its storage key
    LoadSaveFromBrowser(StorageKey),
    /// Export a specific save from the browser to a file on disk
    ExportSaveFromBrowser(StorageKey),

    // =========================================================================
    // Consolidated emulator operations
    // These replace direct FrontendMessage sends from UI components
    // =========================================================================
    /// Power on the console (updates is_powered config)
    PowerOn,
    /// Power off the console (updates is_powered config)
    PowerOff,
    /// Reset the console (soft reset)
    Reset,
    /// Create a manual savestate
    CreateSavestate,
    /// Set the RGB palette and refresh tile textures
    SetPalette(RgbPalette),
    /// Write to PPU palette RAM and request updated palette data
    WritePpuPalette {
        address: u16,
        value: u8,
    },
    /// Write to PPU pattern table and request updated tile data
    WritePpuPattern {
        addr_0: u16,
        value_0: u8,
        addr_1: u16,
        value_1: u8,
    },
    /// Send controller input to the emulator
    ControllerInput(ControllerEvent),
    StepPpuCycle,
    StepCpuCycle,
    StepMasterCycle,
    StepScanline,
    StepFrame,
    Quit,
    PauseEmulator,
    ChangeDebugPalette,
    StartLoadSavestate,
}

/// Context for the multistep savestate loading process
#[derive(Clone)]
pub struct SavestateLoadContext {
    pub savestate: SaveState,
    /// Savestate filename (without path)
    pub savestate_name: String,
    /// Savestate directory (for file picker initial directory)
    pub savestate_dir: Option<String>,
}
