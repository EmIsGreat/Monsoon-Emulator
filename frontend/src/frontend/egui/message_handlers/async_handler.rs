//! Async message handler for frontend operations.
//!
//! This module handles messages from async operations such as file dialogs,
//! savestate loading workflows, and other deferred operations.

use egui::{Context, Id, ViewportCommand};
use monsoon_core::emulation::palette_util::RgbPalette;
use monsoon_core::emulation::ppu_util::EmulatorFetchable;
use monsoon_core::emulation::savestate;
use monsoon_core::util::ToBytes;

use crate::frontend::egui::config::AutoPauseReason;
use crate::frontend::egui::tiles::{add_pane_if_missing, Pane};
use crate::frontend::egui_frontend::EguiApp;
use crate::frontend::messages::{AsyncFrontendMessage, LoadedRom, SavestateLoadContext};
use crate::frontend::savestates::{
    ChecksumMismatchDialogState, ErrorDialogState, MatchingRomDialogState, RomSelectionDialogState,
    SaveBrowserState, SaveEntry, SaveEntryType,
};
use crate::frontend::storage::{Storage, StorageKey};
use crate::frontend::util::{
    spawn_rom_picker, spawn_savestate_picker, try_parse_savestate, SavestateLoadError,
};
use crate::frontend::{storage, util};
use crate::messages::{FrontendMessage, SaveType};

/// Trait for handling async frontend messages.
///
/// This trait is implemented by `EguiApp` to provide message handling
/// in a separate module for better code organization.
pub trait AsyncMessageHandler {
    /// Process all pending async messages from the channel.
    fn handle_async_messages(&mut self, ctx: &Context);
}

impl AsyncMessageHandler for EguiApp {
    fn handle_async_messages(&mut self, ctx: &Context) {
        while let Ok(msg) = self.from_async.try_recv() {
            self.handle_single_async_message(msg, ctx);
        }
    }
}

impl EguiApp {
    /// Handle a single async message.
    pub(crate) fn handle_single_async_message(&mut self, msg: AsyncFrontendMessage, ctx: &Context) {
        match msg {
            AsyncFrontendMessage::PaletteLoaded(loaded) => {
                self.config.user_config.previous_palette_load_dir = Some(loaded.directory);
                self.handle_palette_loaded(ctx, loaded.palette);
            }
            AsyncFrontendMessage::FileSaveCompleted {
                error,
                directory,
                file_type,
            } => {
                if let Some(e) = error {
                    eprintln!("File save error: {}", e);
                }
                if let Some(dir) = directory {
                    match file_type {
                        util::FileType::Palette => {
                            self.config.user_config.previous_palette_save_dir = Some(dir);
                        }
                        util::FileType::Savestate => {
                            self.config.user_config.previous_savestate_save_dir = Some(dir);
                        }
                        // Rom and All don't use save dialogs
                        _ => {}
                    }
                }
            }
            AsyncFrontendMessage::SavestateLoaded(context) => {
                self.handle_savestate_loaded(context);
            }
            AsyncFrontendMessage::ShowMatchingRomDialog(context, matching_rom) => {
                self.config.pending_dialogs.matching_rom_dialog = Some(MatchingRomDialogState {
                    context,
                    matching_rom,
                });
            }
            AsyncFrontendMessage::UseMatchingRom(context, rom) => {
                self.load_savestate_with_rom(&context, rom);
            }
            AsyncFrontendMessage::ManuallySelectRom(context) => {
                util::spawn_rom_picker_for_savestate(
                    &self.async_sender,
                    context,
                    self.config.user_config.previous_rom_load_dir.as_ref(),
                );
            }
            AsyncFrontendMessage::RomSelectedForSavestate(context, rom) => {
                self.handle_rom_selected_for_savestate(&context, rom);
            }
            AsyncFrontendMessage::ShowChecksumMismatchDialog(context, rom) => {
                self.config.pending_dialogs.checksum_mismatch_dialog =
                    Some(ChecksumMismatchDialogState {
                        context,
                        selected_rom: rom,
                    });
            }
            AsyncFrontendMessage::LoadSavestateAnyway(context, rom) => {
                self.load_savestate_with_rom(&context, rom);
            }
            AsyncFrontendMessage::SelectAnotherRom(context) => {
                util::spawn_rom_picker_for_savestate(
                    &self.async_sender,
                    context,
                    self.config.user_config.previous_rom_load_dir.as_ref(),
                );
            }
            AsyncFrontendMessage::SavestateLoadFailed(error) => {
                self.handle_savestate_load_failed(error);
            }
            AsyncFrontendMessage::RomVerificationFailed(context, error) => {
                self.handle_rom_verification_failed(context, error);
            }
            AsyncFrontendMessage::Quickload => {
                self.handle_quickload();
            }
            AsyncFrontendMessage::Quicksave => {
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::CreateSaveState(SaveType::Quicksave));
            }
            AsyncFrontendMessage::LoadRom(loaded_rom) => {
                if let Some(rom) = loaded_rom {
                    let _ = self
                        .to_emulator
                        .send(FrontendMessage::CreateSaveState(SaveType::Autosave));
                    let _ = self.to_emulator.send(FrontendMessage::Power(false));

                    // Save directory for next file picker
                    self.config.user_config.previous_rom_load_dir = Some(rom.directory.clone());

                    self.load_rom(rom);
                    let _ = self.to_emulator.send(FrontendMessage::Power(true));
                    self.config.console_config.is_powered = true;
                }
            }
            AsyncFrontendMessage::OpenSaveBrowser => {
                self.handle_open_save_browser();
            }
            AsyncFrontendMessage::SaveBrowserLoaded(entries) => {
                if let Some(ref mut browser) = self.config.pending_dialogs.save_browser {
                    browser.entries = entries;
                    browser.loading = false;
                }
            }
            AsyncFrontendMessage::LoadSaveFromBrowser(key) => {
                self.handle_load_save_from_browser(key);
            }
            AsyncFrontendMessage::ExportSaveFromBrowser(key) => {
                self.handle_export_save_from_browser(key);
            }
            AsyncFrontendMessage::PowerToggle => {
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::CreateSaveState(SaveType::Autosave));

                self.config.console_config.is_powered = !self.config.console_config.is_powered;

                let _ = self.to_emulator.send(FrontendMessage::Power(
                    self.config.console_config.is_powered,
                ));

                if self.config.console_config.is_powered
                    && let Some((_, rom)) = &self.config.console_config.loaded_rom
                {
                    let name = rom.name.clone();
                    let _ = self
                        .to_emulator
                        .send(FrontendMessage::LoadRom((rom.clone(), name)));
                }
            }
            AsyncFrontendMessage::Reset => {
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::CreateSaveState(SaveType::Autosave));
                let _ = self.to_emulator.send(FrontendMessage::Reset);
            }
            AsyncFrontendMessage::CreateSavestate => {
                if self.config.console_config.loaded_rom.is_some() {
                    self.config
                        .set_auto_pause_reason(AutoPauseReason::SavestateCreateSaveDialog, true);
                }
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::CreateSaveState(SaveType::Manual));
            }
            AsyncFrontendMessage::SetPalette(palette) => {
                self.handle_set_palette(ctx, palette);
            }
            AsyncFrontendMessage::WritePpuPalette {
                address,
                value,
            } => {
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::WritePpu(address, value));
                let _ = self.to_emulator.send(FrontendMessage::RequestDebugData(
                    EmulatorFetchable::Palettes(None),
                ));
            }
            AsyncFrontendMessage::WritePpuPattern {
                addr_0,
                value_0,
                addr_1,
                value_1,
            } => {
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::WritePpu(addr_0, value_0));
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::WritePpu(addr_1, value_1));
                let _ = self.to_emulator.send(FrontendMessage::RequestDebugData(
                    EmulatorFetchable::Tiles(None),
                ));
            }
            AsyncFrontendMessage::ControllerInput(event) => {
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::ControllerInput(event));
            }
            AsyncFrontendMessage::StepPpuCycle => {
                let _ = self.to_emulator.send(FrontendMessage::StepPpuCycle);
            }
            AsyncFrontendMessage::StepCpuCycle => {
                let _ = self.to_emulator.send(FrontendMessage::StepCpuCycle);
            }
            AsyncFrontendMessage::StepMasterCycle => {
                let _ = self.to_emulator.send(FrontendMessage::StepMasterCycle);
            }
            AsyncFrontendMessage::StepScanline => {
                let _ = self.to_emulator.send(FrontendMessage::StepScanline);
            }
            AsyncFrontendMessage::StepFrame => {
                let _ = self.to_emulator.send(FrontendMessage::StepFrame);
            }
            AsyncFrontendMessage::StartLoadRom => spawn_rom_picker(
                &self.async_sender,
                self.config.user_config.previous_rom_load_dir.as_ref(),
            ),
            AsyncFrontendMessage::Quit => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            AsyncFrontendMessage::PauseEmulator => self.pause(),
            AsyncFrontendMessage::AutoPauseSignal {
                signal,
                active,
            } => {
                let reason = match signal {
                    crate::frontend::messages::AutoPauseSignal::SavestateLoadPicker => {
                        AutoPauseReason::SavestateLoadPicker
                    }
                    crate::frontend::messages::AutoPauseSignal::SavestateCreateSaveDialog => {
                        AutoPauseReason::SavestateCreateSaveDialog
                    }
                };
                self.config.set_auto_pause_reason(reason, active);
            }
            AsyncFrontendMessage::ChangeDebugPalette => {
                self.config.user_config.debug_active_palette += 1;
                self.config.user_config.debug_active_palette &= 7;
            }
            AsyncFrontendMessage::StartLoadSavestate => {
                spawn_savestate_picker(
                    &self.async_sender,
                    self.config.user_config.previous_savestate_load_dir.as_ref(),
                );
            }
            AsyncFrontendMessage::PowerCycle => {
                let _ = self.async_sender.send(AsyncFrontendMessage::PowerToggle);
                let _ = self.async_sender.send(AsyncFrontendMessage::PowerToggle);
            }
            AsyncFrontendMessage::OpenKeybindsMenu => {
                add_pane_if_missing(&mut self.tree, Pane::Keybindings);
            }
            AsyncFrontendMessage::OpenOptionsMenu => {
                add_pane_if_missing(&mut self.tree, Pane::Options);
            }
            AsyncFrontendMessage::OpenPaletteViewer => {
                add_pane_if_missing(&mut self.tree, Pane::Palettes);
            }
            AsyncFrontendMessage::OpenPatternTableViewer => {
                add_pane_if_missing(&mut self.tree, Pane::PatternTables);
            }
            AsyncFrontendMessage::OpenNametableViewer => {
                add_pane_if_missing(&mut self.tree, Pane::Nametables);
            }
            AsyncFrontendMessage::OpenSpriteViewer => {
                add_pane_if_missing(&mut self.tree, Pane::Sprites);
            }
            AsyncFrontendMessage::Speedup => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp::<bool>(Id::new(AsyncFrontendMessage::Speedup), true)
                });
            }
        }
        self.config.sync_dialog_pause_reason();
    }

    fn handle_palette_loaded(&mut self, ctx: &Context, palette: RgbPalette) {
        self.config.view_config.palette_rgb_data = palette;
        // Update the renderer's palette
        self.config.view_config.renderer.set_palette(palette);
        // Re-render the current frame with the new palette
        self.emu_textures
            .update_emulator_texture(ctx, &mut self.config.view_config.renderer);
        if self.is_tile_viewer_visible() {
            self.emu_textures.update_tile_textures(
                ctx,
                &self.config.view_config.palette_rgb_data,
                None,
                None,
            );
        }
    }

    fn handle_savestate_loaded(&mut self, context: Box<SavestateLoadContext>) {
        // Try to find a matching ROM by scanning available ROM storage.
        // Only scan if savestate_dir is set (cleared after first scan attempt to
        // prevent loops).
        if context.savestate_dir.is_some() {
            let sender = self.async_sender.clone();
            let context_clone = context.clone();

            // On native, resolve the ROM directory from the storage key
            #[cfg(not(target_arch = "wasm32"))]
            let rom_dir = self
                .config
                .user_config
                .previous_rom_load_dir
                .as_ref()
                .and_then(storage::get_path_for_key)
                .map(|d| d.to_string_lossy().to_string());
            #[cfg(target_arch = "wasm32")]
            let rom_dir: Option<String> = None;

            util::spawn_async(async move {
                let result = find_matching_rom(&context_clone, rom_dir).await;

                if let Some(rom) = result {
                    let _ = sender.send(AsyncFrontendMessage::ShowMatchingRomDialog(
                        context_clone,
                        rom,
                    ));
                } else {
                    // No match found - show ROM selection dialog
                    // Clear savestate_dir to prevent re-scanning
                    let mut context_clone = context_clone;
                    context_clone.savestate_dir = None;
                    let _ = sender.send(AsyncFrontendMessage::SavestateLoaded(context_clone));
                }
            });
            return;
        }

        // Fallback: show ROM selection dialog directly
        self.config.pending_dialogs.rom_selection_dialog = Some(RomSelectionDialogState {
            context,
        });
    }

    fn handle_rom_selected_for_savestate(
        &mut self,
        context: &SavestateLoadContext,
        rom: LoadedRom,
    ) {
        // Save directory for next file picker
        self.config.user_config.previous_rom_load_dir = Some(rom.directory.clone());

        let checksum = util::compute_data_checksum(&rom.data);
        if checksum == context.savestate.rom_file.data_checksum {
            self.load_savestate_with_rom(context, rom);
        } else {
            self.config.pending_dialogs.checksum_mismatch_dialog =
                Some(ChecksumMismatchDialogState {
                    context: Box::new(context.clone()),
                    selected_rom: rom,
                });
        }
    }

    fn handle_savestate_load_failed(&mut self, error: SavestateLoadError) {
        let message = match error {
            SavestateLoadError::FailedToLoadSavestate => {
                "Failed to load or parse the savestate file. The file may be corrupted or invalid."
            }
            SavestateLoadError::FailedToComputeChecksum => {
                "Failed to compute checksum for the ROM file."
            }
        };
        self.config.pending_dialogs.error_dialog = Some(ErrorDialogState {
            title: "Savestate Load Error".to_string(),
            message: message.to_string(),
        });
    }

    fn handle_rom_verification_failed(
        &mut self,
        context: Box<SavestateLoadContext>,
        error: SavestateLoadError,
    ) {
        let message = match error {
            SavestateLoadError::FailedToLoadSavestate => {
                "Unexpected error during ROM verification."
            }
            SavestateLoadError::FailedToComputeChecksum => {
                "Failed to read or compute checksum for the ROM file."
            }
        };
        self.config.pending_dialogs.error_dialog = Some(ErrorDialogState {
            title: "ROM Verification Error".to_string(),
            message: format!("{}\n\nWould you like to select a different ROM?", message),
        });
        self.config.pending_dialogs.rom_selection_dialog = Some(RomSelectionDialogState {
            context,
        });
    }

    fn handle_quickload(&mut self) {
        let rom_info = self
            .config
            .console_config
            .loaded_rom
            .as_ref()
            .map(|r| r.0.data_checksum);
        let rom_name = self.config.user_config.previous_rom_name.clone();
        let sender = self.async_sender.clone();
        let to_emulator = self.to_emulator.clone();

        if let (Some(checksum), Some(prev_name)) = (rom_info, rom_name) {
            let display_name = util::rom_display_name(&prev_name, &checksum);
            util::spawn_async(async move {
                let prefix = storage::quicksaves_prefix(&display_name);
                let storage_impl = storage::get_storage();

                // List quicksaves
                let entries = match storage_impl.list(&prefix).await {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("Failed to list quicksaves: {}", e);
                        let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                            SavestateLoadError::FailedToLoadSavestate,
                        ));
                        return;
                    }
                };

                let key = match EguiApp::find_newest_quicksave(entries) {
                    Some(k) => k,
                    None => return,
                };

                // Read the savestate
                let data = match storage_impl.get(&key).await {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Failed to read quicksave: {}", e);
                        let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                            SavestateLoadError::FailedToLoadSavestate,
                        ));
                        return;
                    }
                };

                let savestate = try_parse_savestate(&sender, &data);

                if savestate.is_none() {
                    return;
                }

                let savestate = savestate.unwrap();

                // Verify checksum
                if checksum != savestate.rom_file.data_checksum {
                    let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                        SavestateLoadError::FailedToLoadSavestate,
                    ));
                    return;
                }

                // ROM verified - load the savestate
                let _ = to_emulator.send(FrontendMessage::LoadSaveState(Box::new(savestate)));
            });
        }
    }

    fn handle_set_palette(&mut self, ctx: &Context, palette: RgbPalette) {
        self.config.view_config.palette_rgb_data = palette;
        // Update the renderer's palette
        self.config.view_config.renderer.set_palette(palette);
        // Re-render the current frame with the new palette
        self.emu_textures
            .update_emulator_texture(ctx, &mut self.config.view_config.renderer);
        if self.is_tile_viewer_visible() {
            self.emu_textures.update_tile_textures(
                ctx,
                &self.config.view_config.palette_rgb_data,
                None,
                None,
            );
        }
    }

    /// Open the save browser dialog and start loading save entries.
    fn handle_open_save_browser(&mut self) {
        if let Some(rom) = &self.config.console_config.loaded_rom
            && let Some(prev_name) = &self.config.user_config.previous_rom_name
        {
            let rom_hash = &rom.0.data_checksum;
            let display_name = util::rom_display_name(prev_name, rom_hash);

            // Show the dialog immediately with loading state
            self.config.pending_dialogs.save_browser = Some(SaveBrowserState {
                entries: Vec::new(),
                game_name: display_name.clone(),
                loading: true,
                show_quicksaves: true,
                show_autosaves: true,
            });

            // List saves asynchronously
            let sender = self.async_sender.clone();
            util::spawn_async(async move {
                let entries = list_save_entries(&display_name).await;
                let _ = sender.send(AsyncFrontendMessage::SaveBrowserLoaded(entries));
            });
        }
    }

    /// Load a save from the browser by reading it from storage.
    fn handle_load_save_from_browser(&mut self, key: StorageKey) {
        // Verify a ROM is loaded and its checksum matches
        let sender = self.async_sender.clone();
        let to_emulator = self.to_emulator.clone();
        let loaded_rom_checksum = self
            .config
            .console_config
            .loaded_rom
            .as_ref()
            .map(|r| r.0.data_checksum);

        util::spawn_async(async move {
            let storage_impl = storage::get_storage();
            match storage_impl.get(&key).await {
                Ok(data) => match savestate::try_load_state_from_bytes(&data) {
                    Some(savestate) => {
                        if let Some(checksum) = loaded_rom_checksum
                            && checksum != savestate.rom_file.data_checksum
                        {
                            let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                                SavestateLoadError::FailedToLoadSavestate,
                            ));
                            return;
                        }
                        let _ =
                            to_emulator.send(FrontendMessage::LoadSaveState(Box::new(savestate)));
                    }
                    None => {
                        let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                            SavestateLoadError::FailedToLoadSavestate,
                        ));
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read save: {}", e);
                    let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                        SavestateLoadError::FailedToLoadSavestate,
                    ));
                }
            }
        });
    }

    /// Export a save from internal storage to a file on disk via save dialog.
    fn handle_export_save_from_browser(&self, key: StorageKey) {
        let sender = self.async_sender.clone();
        let save_dir = self.config.user_config.previous_savestate_save_dir.clone();

        // Read from storage asynchronously, then open save dialog.
        // On native, spawn_save_dialog internally uses tokio::spawn which requires
        // the tokio runtime context, so we read via async Storage instead of sync
        // wrappers.
        util::spawn_async(async move {
            let storage_impl = storage::get_storage();
            match storage_impl.get(&key).await {
                Ok(data) => {
                    let exportable = ExportableData(data);
                    util::spawn_save_dialog(
                        Some(&sender),
                        save_dir.as_ref(),
                        util::FileType::Savestate,
                        Box::new(exportable),
                    );
                }
                Err(e) => {
                    eprintln!("Failed to read save for export: {}", e);
                }
            }
        });
    }
}

/// Try to find a matching ROM by first checking the storage cache (works on
/// both native and WASM), then falling back to scanning the filesystem
/// directory on native.
///
/// On native, `rom_dir` should be the resolved filesystem path of the user's
/// ROM directory. On WASM, `rom_dir` is ignored (always None) and only the
/// IndexedDB cache is searched.
async fn find_matching_rom(
    context: &SavestateLoadContext,
    #[allow(unused)] rom_dir: Option<String>,
) -> Option<LoadedRom> {
    let expected_checksum = &context.savestate.rom_file.data_checksum;
    let storage_impl = storage::get_storage();

    // First, try the storage ROM cache (works on both platforms)
    // Direct lookup by filename if the savestate knows it
    if let Some(ref rom_name) = context.savestate.rom_file.name {
        let direct_key = storage::rom_cache_key(rom_name);
        if let Ok(data) = storage_impl.get(&direct_key).await {
            let checksum = util::compute_data_checksum(&data);
            if &checksum == expected_checksum {
                return Some(LoadedRom {
                    data,
                    name: rom_name.clone(),
                    directory: storage::roms_prefix(),
                });
            }
        }
    }

    // Scan all cached ROMs in storage
    let rom_prefix = storage::roms_prefix();
    if let Ok(entries) = storage_impl.list(&rom_prefix).await {
        for entry in entries {
            if let Ok(data) = storage_impl.get(&entry.key).await {
                let checksum = util::compute_data_checksum(&data);
                if &checksum == expected_checksum {
                    let name = entry
                        .key
                        .sub_path
                        .rsplit('/')
                        .next()
                        .unwrap_or("unknown.nes")
                        .to_string();
                    return Some(LoadedRom {
                        data,
                        name,
                        directory: storage::roms_prefix(),
                    });
                }
            }
        }
    }

    // On native, also scan the filesystem directory as a fallback
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(dir) = rom_dir
        && let Some(rom) = find_matching_rom_in_directory(&dir, context)
    {
        return Some(rom);
    }

    None
}

/// Scan a native filesystem directory for a matching ROM (native only).
#[cfg(not(target_arch = "wasm32"))]
fn find_matching_rom_in_directory(dir: &str, context: &SavestateLoadContext) -> Option<LoadedRom> {
    use std::path::Path;

    use crate::frontend::storage::{StorageCategory, StorageKey};

    let dir_path = Path::new(dir);
    if !dir_path.is_dir() {
        return None;
    }

    let expected_checksum = &context.savestate.rom_file.data_checksum;

    // If the savestate knows the ROM filename, try that first
    if let Some(ref rom_name) = context.savestate.rom_file.name {
        let rom_path = dir_path.join(rom_name);
        if rom_path.is_file()
            && let Ok(data) = std::fs::read(&rom_path)
        {
            let checksum = util::compute_data_checksum(&data);
            if &checksum == expected_checksum {
                return Some(LoadedRom {
                    data,
                    name: rom_name.clone(),
                    directory: StorageKey {
                        category: StorageCategory::Root,
                        sub_path: dir.to_string(),
                    },
                });
            }
        }
    }

    // Scan directory for all .nes files
    let entries = std::fs::read_dir(dir_path).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext.eq_ignore_ascii_case("nes")
            && let Ok(data) = std::fs::read(&path)
        {
            let checksum = util::compute_data_checksum(&data);
            if &checksum == expected_checksum {
                let name = entry.file_name().to_string_lossy().to_string();
                return Some(LoadedRom {
                    data,
                    name,
                    directory: StorageKey {
                        category: StorageCategory::Root,
                        sub_path: dir.to_string(),
                    },
                });
            }
        }
    }

    None
}

/// Wrapper for raw bytes that implements ToBytes for the save dialog export.
struct ExportableData(Vec<u8>);

impl ToBytes for ExportableData {
    fn to_bytes(&self, _format: Option<String>) -> Vec<u8> { self.0.clone() }
}

/// List save entries for a game from storage asynchronously.
async fn list_save_entries(game_name: &str) -> Vec<SaveEntry> {
    let mut entries = Vec::new();

    // List quicksaves
    let qs_prefix = storage::quicksaves_prefix(game_name);
    add_save_entries(&mut entries, &qs_prefix).await;

    // List autosaves
    let as_prefix = storage::autosaves_prefix(game_name);
    add_save_entries(&mut entries, &as_prefix).await;

    // Sort by timestamp descending (newest first)
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries
}

async fn add_save_entries(entries: &mut Vec<SaveEntry>, qs_prefix: &StorageKey) {
    let storage_impl = storage::get_storage();

    if let Ok(qs_entries) = storage_impl.list(qs_prefix).await {
        for entry in qs_entries {
            if entry.key.sub_path.ends_with(".sav")
                && let Some(save_entry) = parse_save_entry(entry.key, SaveEntryType::Quicksave)
            {
                entries.push(save_entry);
            }
        }
    }
}

/// Parse a storage key into a SaveEntry, extracting display name and timestamp.
fn parse_save_entry(key: StorageKey, save_type: SaveEntryType) -> Option<SaveEntry> {
    // Keys look like: saves/<game>/quicksaves/quicksave_2024-01-15_14-30-00.sav
    // or: saves/<game>/autosaves/autosaves_2024-01-15_14-30-00.sav
    let filename = key.sub_path.rsplit('/').next()?;
    let stem = filename.strip_suffix(".sav")?;

    // Split into prefix and timestamp: "quicksave_2024-01-15_14-30-00" or with
    // version suffix
    let (_prefix, rest) = stem.split_once('_')?;

    // Try to extract timestamp - handle optional version suffix
    let timestamp_str = extract_timestamp(rest)?.0;

    // Parse the timestamp: "2024-01-15_14-30-00" → "2024/01/15 14:30:00"
    let timestamp = if let Some((date, time)) = timestamp_str.split_once('_') {
        // Date: 2024-01-15 → 2024/01/15
        let date_display = date.replacen('-', "/", 2);
        // Time: 14-30-00 → 14:30:00
        let time_display = time.replacen('-', ":", 2);
        format!("{} {}", date_display, time_display)
    } else {
        timestamp_str.to_string()
    };

    // Display name uses the timestamp for uniqueness
    let display_name = format!("{}", save_type);

    Some(SaveEntry {
        key,
        display_name,
        timestamp,
        save_type,
    })
}

pub fn extract_timestamp(rest: &str) -> Option<(&str, &str)> {
    let (timestamp_str, version) = if rest.chars().filter(|c| *c == '_').count() > 1 {
        rest.rsplit_once('_')?
    } else {
        (rest, "0")
    };
    Some((timestamp_str, version))
}
