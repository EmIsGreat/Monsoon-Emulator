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
};
use crate::frontend::egui_frontend::EguiApp;
use crate::frontend::messages::{AsyncFrontendMessage, LoadedRom, SavestateLoadContext};
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
            AsyncFrontendMessage::FileSaveCompleted(error) => {
                if let Some(e) = error {
                    eprintln!("File save error: {}", e);
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
        // On native, try to find a matching ROM in the savestate's directory
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ref dir) = context.savestate_dir {
            let sender = self.async_sender.clone();
            let context_clone = context.clone();
            let dir = dir.clone();
            std::thread::spawn(move || {
                if let Some(rom) = find_matching_rom_in_directory(&dir, &context_clone) {
                    let _ = sender.send(AsyncFrontendMessage::ShowMatchingRomDialog(
                        context_clone,
                        rom,
                    ));
                } else {
                    // No match found - show ROM selection dialog
                    // Clear savestate_dir to prevent re-scanning
                    let mut ctx = *context_clone;
                    ctx.savestate_dir = None;
                    let _ = sender.send(AsyncFrontendMessage::SavestateLoaded(Box::new(ctx)));
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
                    // Checksum mismatch - show error dialog
                    self.config.pending_dialogs.error_dialog = Some(ErrorDialogState {
                        title: "Quickload Error".to_string(),
                        message: "The current ROM doesn't match the savestate's ROM. Please load the correct ROM first.".to_string(),
                    });
                    return;
                }
            } else {
                // No ROM loaded - show error dialog
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
}

/// Try to find a ROM in the given directory whose checksum matches the savestate's expected ROM.
///
/// On native only: scans the directory for .nes files. If the savestate knows the ROM's filename,
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
