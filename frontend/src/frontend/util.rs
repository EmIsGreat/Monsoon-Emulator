use std::future::Future;

use crossbeam_channel::Sender;
use monsoon_core::emulation::palette_util::parse_palette_from_bytes;
use monsoon_core::emulation::savestate;
use monsoon_core::emulation::savestate::SaveState;
use monsoon_core::util::ToBytes;
use rfd::{AsyncFileDialog, FileHandle};
use sha2::{Digest, Sha256};

use crate::frontend::messages::{
    AsyncFrontendMessage, LoadedPalette, LoadedRom, SavestateLoadContext,
};
use crate::frontend::storage::{self, Storage, StorageCategory, StorageKey, get_storage};


#[derive(Clone, Copy, Eq, PartialEq)]
/// Enum to represent errors that can occur during savestate loading UI flow
pub enum SavestateLoadError {
    /// Failed to load or parse the savestate file
    FailedToLoadSavestate,
    /// Failed to compute checksum for a ROM file
    FailedToComputeChecksum,
}

/// Extract the parent directory from a FileHandle.
/// On native, this uses the file's path. On WASM, returns None.
#[cfg(not(target_arch = "wasm32"))]
fn get_file_directory(handle: &FileHandle) -> Option<String> {
    handle
        .path()
        .parent()
        .map(|p| p.to_string_lossy().to_string())
}

#[cfg(target_arch = "wasm32")]
fn get_file_directory(_handle: &FileHandle) -> Option<String> {
    // WASM doesn't have filesystem paths
    None
}

pub trait Contrastable {
    fn get_contrast(&self) -> Self;
}

impl Contrastable for egui::Color32 {
    /// Calculate foreground color (black or white) based on background
    /// luminance
    fn get_contrast(&self) -> Self {
        let bg = self.as_u32();
        let r = (bg >> 16) & 0xFF;
        let g = (bg >> 8) & 0xFF;
        let b = bg & 0xFF;

        let luminance = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;

        if luminance > 128.0 {
            egui::Color32::BLACK
        } else {
            egui::Color32::WHITE
        }
    }
}

pub trait FromU32 {
    fn from_u32(d: u32) -> Self;
}

pub trait AsU32 {
    fn as_u32(&self) -> u32;
}

impl FromU32 for egui::Color32 {
    fn from_u32(d: u32) -> Self {
        egui::Color32::from_rgba_unmultiplied(
            (d >> 16) as u8,
            (d >> 8) as u8,
            d as u8,
            (d >> 24) as u8,
        )
    }
}

impl AsU32 for egui::Color32 {
    fn as_u32(&self) -> u32 {
        ((self.a() as u32) << 24)
            | ((self.r() as u32) << 16)
            | ((self.g() as u32) << 8)
            | (self.b() as u32)
    }
}

/// Pick a file using the async file dialog.
/// The directory hint is used to set the initial directory if provided.
pub async fn pick_file(file_type: FileType, directory: Option<&StorageKey>) -> Option<FileHandle> {
    let mut dialog = AsyncFileDialog::new()
        .add_filetype_filter(file_type)
        .add_filetype_filter(FileType::All);

    if let Some(dir) = get_storage().key_to_path_opt(directory) {
        dialog = dialog.set_directory(dir);
    }

    dialog.pick_file().await
}

/// Save a file using the async file dialog.
/// The directory hint is used to set the initial directory if provided.
pub async fn save_file(file_type: FileType, directory: Option<&StorageKey>) -> Option<FileHandle> {
    let mut dialog = AsyncFileDialog::new()
        .add_filetype_filter(file_type)
        .add_filetype_filter(FileType::All)
        .set_can_create_directories(true);

    if let Some(dir) = get_storage().key_to_path_opt(directory) {
        dialog = dialog.set_directory(dir);
    }

    dialog.save_file().await
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum FileType {
    Rom,
    Savestate,
    Palette,
    All,
}

impl FileType {
    pub fn add_filters(&self, dialog: AsyncFileDialog) -> AsyncFileDialog {
        match self {
            FileType::Rom => dialog.add_filter("NES ROM File", &[self.get_default_extension()]),
            FileType::Savestate => {
                dialog.add_filter("Savestate", &[self.get_default_extension(), "json"])
            }
            FileType::Palette => {
                dialog.add_filter("NES Palette File", &[self.get_default_extension()])
            }
            FileType::All => dialog.add_filter("All Files", &["*"]),
        }
    }

    pub fn get_default_extension(&self) -> &str {
        match self {
            FileType::Rom => "nes",
            FileType::Savestate => "sav",
            FileType::Palette => "pal",
            FileType::All => "",
        }
    }
}

pub trait AddFilter {
    fn add_filetype_filter(self, file_type: FileType) -> Self;
}

impl AddFilter for AsyncFileDialog {
    fn add_filetype_filter(self, file_type: FileType) -> Self { file_type.add_filters(self) }
}

/// Spawn a file picker for palette files that loads the palette asynchronously.
///
/// This reads and parses the palette file asynchronously to avoid blocking the
/// UI.
///
/// # Arguments
/// * `sender` - Channel to send the loaded palette back to the UI thread
/// * `directory` - Optional directory hint for the file picker
pub fn spawn_palette_picker(sender: &Sender<AsyncFrontendMessage>, dir: Option<&StorageKey>) {
    let sender = sender.clone();
    let dir = dir.cloned();

    spawn_async(async move {
        if let Some(handle) = pick_file(FileType::Palette, dir.as_ref()).await {
            // Read the file contents from the handle
            let data = handle.read().await;
            let palette = parse_palette_from_bytes(&data);
            let directory = get_file_directory(&handle)
                .map(|f| StorageKey::from(&f))
                .unwrap_or(StorageKey {
                    category: StorageCategory::Cache,
                    sub_path: "upload_cache/palettes/".to_string(),
                });
            let _ = sender.send(AsyncFrontendMessage::PaletteLoaded(LoadedPalette {
                palette,
                directory,
            }));
        }
    });
}

/// Spawn a file picker for ROM files that loads the ROM data asynchronously.
///
/// # Arguments
/// * `sender` - Channel to send the loaded ROM back to the UI thread
/// * `directory` - Optional directory hint for the file picker
pub fn spawn_rom_picker(sender: &Sender<AsyncFrontendMessage>, dir: Option<&StorageKey>) {
    let sender = sender.clone();
    let dir = dir.cloned();

    spawn_async(async move {
        if let Some(handle) = pick_file(FileType::Rom, dir.as_ref()).await {
            let data = handle.read().await;
            let name = handle.file_name();
            let directory = get_file_directory(&handle)
                .map(|f| StorageKey::from(&f))
                .unwrap_or(StorageKey {
                    category: StorageCategory::Cache,
                    sub_path: "upload_cache/roms/".to_string(),
                });

            // Cache ROM in storage for later access (ROM matching, etc.)
            let cache_key = storage::rom_cache_key(&name);
            let _ = get_storage().set(&cache_key, data.clone()).await;

            let _ = sender.send(AsyncFrontendMessage::LoadRom(Some(LoadedRom {
                data,
                name,
                directory,
            })));
        }
    });
}

/// Spawn a save dialog for a file asynchronously.
/// The file is written after the user selects a path.
///
/// # Arguments
/// * `sender` - Optional channel to notify completion
/// * `directory` - Optional directory hint for the file picker
/// * `file_type` - Type of file being saved
/// * `data` - Data to write to the file
pub fn spawn_save_dialog(
    sender: Option<&Sender<AsyncFrontendMessage>>,
    dir: Option<&StorageKey>,
    file_type: FileType,
    data: Box<dyn ToBytes + Send>,
) {
    let sender = sender.cloned();
    let dir = dir.cloned();
    spawn_async(async move {
        if let Some(handle) = save_file(file_type, dir.as_ref()).await {
            // Get filename for format detection
            let filename = handle.file_name();
            let format = get_extension(&filename);

            // If no extension was specified, use the default for this file type
            let format = format.or_else(|| {
                let ext = file_type.get_default_extension();
                if ext.is_empty() {
                    None
                } else {
                    Some(ext.to_string())
                }
            });

            // Capture directory from the save handle
            let save_dir = get_file_directory(&handle).map(|f| StorageKey::from(&f));

            // Write data using the file handle
            let bytes = data.to_bytes(format);

            // On native, if the filename had no extension, write to a path with the
            // default extension appended so the file on disk gets the correct name.
            #[cfg(not(target_arch = "wasm32"))]
            let result = {
                if get_extension(&filename).is_none() {
                    let ext = file_type.get_default_extension();
                    if !ext.is_empty() {
                        let path = handle.path().with_extension(ext);
                        std::fs::write(&path, &bytes)
                    } else {
                        handle.write(&bytes).await
                    }
                } else {
                    handle.write(&bytes).await
                }
            };

            #[cfg(target_arch = "wasm32")]
            let result = handle.write(&bytes).await;

            // Notify completion if a sender was provided
            let error = result.err().map(|e| format!("Failed to write file: {}", e));
            if let Some(sender) = sender {
                let _ = sender.send(AsyncFrontendMessage::FileSaveCompleted {
                    error,
                    directory: save_dir,
                    file_type,
                });
            }
        }
    });
}

/// Extract the file extension from a filename
fn get_extension(filename: &str) -> Option<String> {
    filename.rsplit_once('.').map(|(_, ext)| ext.to_string())
}

pub fn color_radio<Value: PartialEq>(
    ui: &mut egui::Ui,
    current: &mut Value,
    alternative: Value,
    color: egui::Color32,
) -> egui::Response {
    let size = egui::vec2(20.0, 20.0);
    let selected = *current == alternative;

    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Draw the colored square
        painter.rect_filled(rect, 2.0, color);

        // Draw selection border when selected
        if selected {
            painter.rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(2.0, ui.visuals().selection.stroke.color),
                egui::StrokeKind::Inside,
            );
        }
    }

    if response.clicked() {
        *current = alternative;
    }

    response
}

/// Compute SHA256 checksum for data
pub fn compute_data_checksum(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Spawn the initial savestate file picker asynchronously.
/// After the savestate is loaded, it will prompt for ROM selection.
///
/// # Arguments
/// * `sender` - Channel to send the loaded savestate back to the UI thread
/// * `directory` - Optional directory hint for the file picker
pub fn spawn_savestate_picker(sender: &Sender<AsyncFrontendMessage>, dir: Option<&StorageKey>) {
    let sender = sender.clone();
    let dir = dir.cloned();

    spawn_async(async move {
        let handle = pick_file(FileType::Savestate, dir.as_ref()).await;

        if let Some(handle) = handle {
            // Read savestate data from the file handle
            let data = handle.read().await;
            let savestate_name = handle.file_name();
            let savestate_dir = get_file_directory(&handle);

            // Cache savestate in storage for later access
            let cache_key = storage::uploaded_savestate_key(&savestate_name);
            let _ = get_storage().set(&cache_key, data.clone()).await;

            // Try to parse the savestate from the data
            let savestate = match try_parse_savestate(&sender, &data) {
                Some(value) => value,
                None => return,
            };

            let context = SavestateLoadContext {
                savestate,
                savestate_name,
                savestate_dir,
            };

            // Send context for next step - user will need to select ROM
            let _ = sender.send(AsyncFrontendMessage::SavestateLoaded(Box::new(context)));
        }
    });
}

pub fn try_parse_savestate(
    sender: &Sender<AsyncFrontendMessage>,
    data: &[u8],
) -> Option<SaveState> {
    let savestate = match savestate::try_load_state_from_bytes(data) {
        Some(s) => s,
        None => {
            // Failed to load savestate - send error notification
            let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                SavestateLoadError::FailedToLoadSavestate,
            ));
            return None;
        }
    };
    Some(savestate)
}

/// Spawn a file picker to select a ROM for a savestate
///
/// # Arguments
/// * `sender` - Channel to send the loaded ROM back to the UI thread
/// * `context` - Savestate context for the ROM selection
/// * `directory` - Optional directory hint for the file picker
pub fn spawn_rom_picker_for_savestate(
    sender: &Sender<AsyncFrontendMessage>,
    context: Box<SavestateLoadContext>,
    directory: Option<&StorageKey>,
) {
    let sender = sender.clone();
    let dir = directory.cloned();
    spawn_async(async move {
        let handle = pick_file(FileType::Rom, dir.as_ref()).await;

        if let Some(handle) = handle {
            let data = handle.read().await;
            let name = handle.file_name();
            let directory = get_file_directory(&handle)
                .map(|f| StorageKey::from(&f))
                .unwrap_or(StorageKey {
                    category: StorageCategory::Cache,
                    sub_path: "upload_cache/roms/".to_string(),
                });

            // Cache ROM in storage for later access (ROM matching, etc.)
            let cache_key = storage::rom_cache_key(&name);
            let _ = get_storage().set(&cache_key, data.clone()).await;

            let _ = sender.send(AsyncFrontendMessage::RomSelectedForSavestate(
                context,
                LoadedRom {
                    data,
                    name,
                    directory,
                },
            ));
        }
    });
}

/// Get a display name for a ROM based on its name and checksum
pub fn rom_display_name(name: &str, sha256: &[u8; 32]) -> String {
    // Extract stem (filename without extension)
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    if !stem.is_empty() {
        stem.to_owned()
    } else {
        format!("Unknown ROM ({})", short_hash_hex(sha256))
    }
}

pub fn short_hash_hex(hash: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(12);
    for b in &hash[..6] {
        write!(&mut s, "{:02x}", b).unwrap();
    }
    s
}

#[cfg(target_arch = "wasm32")]
pub fn spawn_async<F: Future<Output = ()> + 'static>(f: F) { wasm_bindgen_futures::spawn_local(f); }

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_async<F: Future<Output = ()> + Send + 'static>(f: F) { tokio::spawn(f); }
