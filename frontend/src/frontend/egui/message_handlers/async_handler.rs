//! Async message handler for frontend operations.
//!
//! This module handles messages from async operations such as file dialogs,
//! savestate loading workflows, and other deferred operations.

use egui::Context;
use ensemble_lockstep::emulation::ppu::EmulatorFetchable;
use ensemble_lockstep::emulation::savestate;
use ensemble_lockstep::emulation::screen_renderer::RgbPalette;

use crate::frontend::egui::config::{
    ChecksumMismatchDialogState, ErrorDialogState, MatchingRomDialogState, RomSelectionDialogState,
    SaveBrowserState, SaveEntry, SaveEntryType,
};
use crate::frontend::egui_frontend::EguiApp;
use crate::frontend::messages::{AsyncFrontendMessage, LoadedRom, SavestateLoadContext};
#[cfg(target_arch = "wasm32")]
use crate::frontend::storage::Storage;
use crate::frontend::util::SavestateLoadError;
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
                self.config.user_config.previous_palette_dir = Some(loaded.directory);
                self.handle_palette_loaded(ctx, loaded.palette);
            }
            AsyncFrontendMessage::FileSaveCompleted { error, directory, file_type } => {
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
                self.load_savestate_with_rom(&context, rom.data, rom.name);
            }
            AsyncFrontendMessage::ManuallySelectRom(context) => {
                util::spawn_rom_picker_for_savestate(
                    &self.async_sender,
                    context,
                    self.config.user_config.previous_rom_dir.as_ref(),
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
                self.load_savestate_with_rom(&context, rom.data, rom.name);
            }
            AsyncFrontendMessage::SelectAnotherRom(context) => {
                util::spawn_rom_picker_for_savestate(
                    &self.async_sender,
                    context,
                    self.config.user_config.previous_rom_dir.as_ref(),
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
                    let _ = self.to_emulator.send(FrontendMessage::PowerOff);

                    // Save directory for next file picker
                    self.config.user_config.previous_rom_dir =
                        Some(rom.directory);

                    self.load_rom(rom.data, rom.name);
                    let _ = self.to_emulator.send(FrontendMessage::Power);
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
            AsyncFrontendMessage::PowerOn => {
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::CreateSaveState(SaveType::Autosave));
                let _ = self.to_emulator.send(FrontendMessage::Power);
                self.config.console_config.is_powered = true;
            }
            AsyncFrontendMessage::PowerOff => {
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::CreateSaveState(SaveType::Autosave));
                let _ = self.to_emulator.send(FrontendMessage::PowerOff);
                self.config.console_config.is_powered = false;
            }
            AsyncFrontendMessage::Reset => {
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::CreateSaveState(SaveType::Autosave));
                let _ = self.to_emulator.send(FrontendMessage::Reset);
            }
            AsyncFrontendMessage::CreateSavestate => {
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
        }
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
        // On native, try to find a matching ROM in the last-used ROM directory.
        // Only scan if savestate_dir is set (cleared after first scan attempt to prevent loops).
        #[cfg(not(target_arch = "wasm32"))]
        if context.savestate_dir.is_some() {
            if let Some(dir) = self
                .config
                .user_config
                .previous_rom_dir
                .as_ref()
                .and_then(storage::get_path_for_key)
            {
                let dir = dir.to_string_lossy().to_string();
                let sender = self.async_sender.clone();
                let context_clone = context.clone();
                std::thread::spawn(move || {
                    if let Some(rom) = find_matching_rom_in_directory(&dir, &context_clone) {
                        let _ = sender.send(AsyncFrontendMessage::ShowMatchingRomDialog(
                            context_clone,
                            rom,
                        ));
                    } else {
                        // No match found - show ROM selection dialog
                        // Clear savestate_dir to prevent re-scanning
                        let mut context_clone = context_clone;
                        context_clone.savestate_dir = None;
                        let _ =
                            sender.send(AsyncFrontendMessage::SavestateLoaded(context_clone));
                    }
                });
                return;
            }
        }

        // On WASM, try to find a matching ROM in the IndexedDB ROM cache.
        // Only scan if savestate_dir is set (cleared after first scan attempt to prevent loops).
        #[cfg(target_arch = "wasm32")]
        if context.savestate_dir.is_some() {
            let sender = self.async_sender.clone();
            let context_clone = context.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(rom) = find_matching_rom_in_storage(&context_clone).await {
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
        self.config.user_config.previous_rom_dir = Some(rom.directory.clone());

        let checksum = util::compute_data_checksum(&rom.data);
        if checksum == context.savestate.rom_file.data_checksum {
            self.load_savestate_with_rom(context, rom.data, rom.name);
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
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(key) = self.get_current_quicksave_key() {
                // Read the savestate using storage
                let data = match storage::read_sync(&key) {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("Failed to read quicksave file: {}", e);
                        let _ = self
                            .async_sender
                            .send(AsyncFrontendMessage::SavestateLoadFailed(
                                SavestateLoadError::FailedToLoadSavestate,
                            ));
                        return;
                    }
                };

                let savestate = match savestate::try_load_state_from_bytes(&data) {
                    Some(s) => s,
                    None => {
                        let _ = self
                            .async_sender
                            .send(AsyncFrontendMessage::SavestateLoadFailed(
                                SavestateLoadError::FailedToLoadSavestate,
                            ));
                        return;
                    }
                };

                // Verify the currently loaded ROM matches the savestate's ROM checksum
                if let Some(loaded_rom) = &self.config.user_config.loaded_rom {
                    if loaded_rom.data_checksum != savestate.rom_file.data_checksum {
                        self.config.pending_dialogs.error_dialog = Some(ErrorDialogState {
                            title: "Quickload Error".to_string(),
                            message: "The current ROM doesn't match the savestate's ROM. Please load the correct ROM first.".to_string(),
                        });
                        return;
                    }
                } else {
                    self.config.pending_dialogs.error_dialog = Some(ErrorDialogState {
                        title: "Quickload Error".to_string(),
                        message: "No ROM is currently loaded. Please load a ROM first.".to_string(),
                    });
                    return;
                }

                // ROM verified - load the savestate
                let _ = self
                    .to_emulator
                    .send(FrontendMessage::LoadSaveState(Box::new(savestate)));
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // On WASM, we need to do everything async
            let rom_info = self.config.user_config.loaded_rom.as_ref().map(|r| r.data_checksum);
            let rom_name = self.config.user_config.previous_rom_name.clone();
            let sender = self.async_sender.clone();
            let to_emulator = self.to_emulator.clone();

            if let (Some(checksum), Some(prev_name)) = (rom_info, rom_name) {
                let display_name = util::rom_display_name(&prev_name, &checksum);
                wasm_bindgen_futures::spawn_local(async move {
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

                    let savestate = match savestate::try_load_state_from_bytes(&data) {
                        Some(s) => s,
                        None => {
                            let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                                SavestateLoadError::FailedToLoadSavestate,
                            ));
                            return;
                        }
                    };

                    // Verify checksum
                    if checksum != savestate.rom_file.data_checksum {
                        // Can't show dialog from async context, send error via channel
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
        if let Some(rom) = &self.config.user_config.loaded_rom
            && let Some(prev_name) = &self.config.user_config.previous_rom_name
        {
            let rom_hash = &rom.data_checksum;
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
            #[cfg(not(target_arch = "wasm32"))]
            {
                std::thread::spawn(move || {
                    let entries = list_save_entries(&display_name);
                    let _ = sender.send(AsyncFrontendMessage::SaveBrowserLoaded(entries));
                });
            }
            #[cfg(target_arch = "wasm32")]
            {
                wasm_bindgen_futures::spawn_local(async move {
                    let entries = list_save_entries_async(&display_name).await;
                    let _ = sender.send(AsyncFrontendMessage::SaveBrowserLoaded(entries));
                });
            }
        }
    }

    /// Load a save from the browser by reading it from storage.
    fn handle_load_save_from_browser(&mut self, key: storage::StorageKey) {
        // Verify a ROM is loaded and its checksum matches
        let sender = self.async_sender.clone();
        let to_emulator = self.to_emulator.clone();
        let loaded_rom_checksum = self.config.user_config.loaded_rom.as_ref().map(|r| r.data_checksum);

        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                match storage::read_sync(&key) {
                    Ok(data) => {
                        match savestate::try_load_state_from_bytes(&data) {
                            Some(savestate) => {
                                // Verify checksum
                                if let Some(checksum) = loaded_rom_checksum {
                                    if checksum != savestate.rom_file.data_checksum {
                                        let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                                            SavestateLoadError::FailedToLoadSavestate,
                                        ));
                                        return;
                                    }
                                }
                                let _ = to_emulator.send(FrontendMessage::LoadSaveState(Box::new(savestate)));
                            }
                            None => {
                                let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                                    SavestateLoadError::FailedToLoadSavestate,
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read save: {}", e);
                        let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                            SavestateLoadError::FailedToLoadSavestate,
                        ));
                    }
                }
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let storage_impl = storage::get_storage();
                match storage_impl.get(&key).await {
                    Ok(data) => {
                        match savestate::try_load_state_from_bytes(&data) {
                            Some(savestate) => {
                                if let Some(checksum) = loaded_rom_checksum {
                                    if checksum != savestate.rom_file.data_checksum {
                                        let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                                            SavestateLoadError::FailedToLoadSavestate,
                                        ));
                                        return;
                                    }
                                }
                                let _ = to_emulator.send(FrontendMessage::LoadSaveState(Box::new(savestate)));
                            }
                            None => {
                                let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                                    SavestateLoadError::FailedToLoadSavestate,
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read save: {}", e);
                        let _ = sender.send(AsyncFrontendMessage::SavestateLoadFailed(
                            SavestateLoadError::FailedToLoadSavestate,
                        ));
                    }
                }
            });
        }
    }

    /// Export a save from internal storage to a file on disk via save dialog.
    fn handle_export_save_from_browser(&self, key: storage::StorageKey) {
        let sender = self.async_sender.clone();
        let save_dir = self.config.user_config.previous_savestate_save_dir.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                match storage::read_sync(&key) {
                    Ok(data) => {
                        // Use an ExportableData wrapper to pass raw bytes to spawn_save_dialog
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

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
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
}

/// Try to find a ROM in the given directory whose checksum matches the savestate's expected ROM.
///
/// Scans the directory for .nes files. If the savestate knows the ROM's filename,
/// that file is checked first. Then all other .nes files are checked.
#[cfg(not(target_arch = "wasm32"))]
fn find_matching_rom_in_directory(
    dir: &str,
    context: &SavestateLoadContext,
) -> Option<LoadedRom> {
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
        if rom_path.is_file() {
            if let Ok(data) = std::fs::read(&rom_path) {
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
    }

    // Scan directory for all .nes files
    let entries = std::fs::read_dir(dir_path).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("nes") {
                    if let Ok(data) = std::fs::read(&path) {
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
            }
        }
    }

    None
}

/// Try to find a ROM in the IndexedDB cache whose checksum matches the savestate's expected ROM.
///
/// Scans all cached ROMs in IndexedDB. If the savestate knows the ROM's filename,
/// that file is checked first for efficiency.
#[cfg(target_arch = "wasm32")]
async fn find_matching_rom_in_storage(
    context: &SavestateLoadContext,
) -> Option<LoadedRom> {
    let expected_checksum = &context.savestate.rom_file.data_checksum;
    let storage_impl = storage::get_storage();

    // If the savestate knows the ROM filename, try a direct lookup first
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

    // Scan all cached ROMs
    let rom_prefix = storage::roms_prefix();
    if let Ok(entries) = storage_impl.list(&rom_prefix).await {
        for entry in entries {
            if let Ok(data) = storage_impl.get(&entry.key).await {
                let checksum = util::compute_data_checksum(&data);
                if &checksum == expected_checksum {
                    let name = entry.key.sub_path
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

    None
}

/// Wrapper for raw bytes that implements ToBytes for the save dialog export.
struct ExportableData(Vec<u8>);

impl ensemble_lockstep::util::ToBytes for ExportableData {
    fn to_bytes(&self, _format: Option<String>) -> Vec<u8> {
        self.0.clone()
    }
}

/// List save entries for a game from storage (native, synchronous).
#[cfg(not(target_arch = "wasm32"))]
fn list_save_entries(game_name: &str) -> Vec<SaveEntry> {
    let mut entries = Vec::new();

    // List quicksaves
    let qs_prefix = storage::quicksaves_prefix(game_name);
    if let Ok(qs_entries) = storage::list_sync(&qs_prefix) {
        for entry in qs_entries {
            if entry.key.sub_path.ends_with(".sav") {
                if let Some(save_entry) = parse_save_entry(entry.key, SaveEntryType::Quicksave) {
                    entries.push(save_entry);
                }
            }
        }
    }

    // List autosaves
    let as_prefix = storage::autosaves_prefix(game_name);
    if let Ok(as_entries) = storage::list_sync(&as_prefix) {
        for entry in as_entries {
            if entry.key.sub_path.ends_with(".sav") {
                if let Some(save_entry) = parse_save_entry(entry.key, SaveEntryType::Autosave) {
                    entries.push(save_entry);
                }
            }
        }
    }

    // Sort by timestamp descending (newest first)
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries
}

/// List save entries for a game from storage (WASM, asynchronous).
#[cfg(target_arch = "wasm32")]
async fn list_save_entries_async(game_name: &str) -> Vec<SaveEntry> {
    let mut entries = Vec::new();
    let storage_impl = storage::get_storage();

    // List quicksaves
    let qs_prefix = storage::quicksaves_prefix(game_name);
    if let Ok(qs_entries) = storage_impl.list(&qs_prefix).await {
        for entry in qs_entries {
            if entry.key.sub_path.ends_with(".sav") {
                if let Some(save_entry) = parse_save_entry(entry.key, SaveEntryType::Quicksave) {
                    entries.push(save_entry);
                }
            }
        }
    }

    // List autosaves
    let as_prefix = storage::autosaves_prefix(game_name);
    if let Ok(as_entries) = storage_impl.list(&as_prefix).await {
        for entry in as_entries {
            if entry.key.sub_path.ends_with(".sav") {
                if let Some(save_entry) = parse_save_entry(entry.key, SaveEntryType::Autosave) {
                    entries.push(save_entry);
                }
            }
        }
    }

    // Sort by timestamp descending (newest first)
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries
}

/// Parse a storage key into a SaveEntry, extracting display name and timestamp.
fn parse_save_entry(key: storage::StorageKey, save_type: SaveEntryType) -> Option<SaveEntry> {
    // Keys look like: saves/<game>/quicksaves/quicksave_2024-01-15_14-30-00.sav
    // or: saves/<game>/autosaves/autosaves_2024-01-15_14-30-00.sav
    let filename = key.sub_path.rsplit('/').next()?;
    let stem = filename.strip_suffix(".sav")?;

    // Split into prefix and timestamp: "quicksave_2024-01-15_14-30-00" or with version suffix
    let (prefix, rest) = stem.split_once('_')?;

    // Try to extract timestamp - handle optional version suffix
    let (timestamp_str, _version) = if rest.chars().filter(|c| *c == '_').count() > 1 {
        rest.rsplit_once('_')?
    } else {
        (rest, "0")
    };

    // Format display name
    let display_name = match save_type {
        SaveEntryType::Quicksave => format!("Quicksave ({})", prefix),
        SaveEntryType::Autosave => format!("Autosave ({})", prefix),
    };

    // Format timestamp for display
    let timestamp = timestamp_str
        .replace('_', " ")
        .replacen('-', "/", 2)
        .replacen('-', ":", 2);

    Some(SaveEntry {
        key,
        display_name,
        timestamp,
        save_type,
    })
}
