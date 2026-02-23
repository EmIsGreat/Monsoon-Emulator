//! Emulator message handler for backend communication.
//!
//! This module handles messages from the emulator backend, including
//! frame updates, debug data, and savestate operations.

use egui::{Context, ViewportCommand};
use ensemble_lockstep::emulation::ppu::{EmulatorFetchable, PaletteData, TileData, TILE_COUNT};
use ensemble_lockstep::emulation::savestate::SaveState;
use ensemble_lockstep::util::ToBytes;

use crate::frontend::egui_frontend::EguiApp;
#[cfg(target_arch = "wasm32")]
use crate::frontend::storage::Storage;
use crate::frontend::util::FileType;
use crate::frontend::{storage, util};
use crate::messages::{EmulatorMessage, SaveType};

/// Trait for handling emulator messages.
///
/// This trait is implemented by `EguiApp` to provide message handling
/// in a separate module for better code organization.
pub trait EmulatorMessageHandler {
    /// Process all pending emulator messages from the channel.
    fn handle_emulator_messages(&mut self, ctx: &Context);
}

impl EmulatorMessageHandler for EguiApp {
    fn handle_emulator_messages(&mut self, ctx: &Context) {
        while let Ok(msg) = self.from_emulator.try_recv() {
            self.handle_single_emulator_message(msg, ctx);
        }
    }
}

impl EguiApp {
    /// Handle a single emulator message.
    pub(crate) fn handle_single_emulator_message(&mut self, msg: EmulatorMessage, ctx: &Context) {
        match msg {
            EmulatorMessage::FrameReady(frame) => {
                self.emu_textures.current_frame = Some(frame);
                self.fps_counter.update();
                self.emu_textures
                    .update_emulator_texture(ctx, &mut self.config.view_config.renderer);
            }
            EmulatorMessage::DebugData(data) => {
                self.handle_debug_data(ctx, data);
            }
            EmulatorMessage::Stopped => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            EmulatorMessage::SaveState(s, t) => {
                self.handle_savestate(s, t);
            }
            EmulatorMessage::RomLoaded(rom) => {
                self.config.user_config.loaded_rom = rom;
            }
        }
    }

    fn handle_debug_data(&mut self, ctx: &Context, data: EmulatorFetchable) {
        match data {
            EmulatorFetchable::Palettes(p) => {
                self.handle_palette_data(ctx, p);
            }
            EmulatorFetchable::Tiles(t) => {
                self.handle_tile_data(ctx, t);
            }
            EmulatorFetchable::Nametables(n) => {
                self.emu_textures.nametable_data = n;
            }
        }
    }

    fn handle_palette_data(&mut self, ctx: &Context, new_palette_data: Option<Box<PaletteData>>) {
        // Only rebuild textures if palette data actually changed and a tile viewer is visible
        if self.emu_textures.palette_data != new_palette_data {
            let changed_palettes = self.detect_changed_palettes(&new_palette_data);
            self.emu_textures.palette_data = new_palette_data;

            if self.is_tile_viewer_visible() {
                for palette_idx in changed_palettes {
                    self.emu_textures.update_tile_textures(
                        ctx,
                        &self.config.view_config.palette_rgb_data,
                        Some(palette_idx),
                        None,
                    );
                }
            }
        }
    }

    fn handle_tile_data(
        &mut self,
        ctx: &Context,
        new_tile_data: Option<Box<[TileData; TILE_COUNT]>>,
    ) {
        let changed_tiles = self.detect_changed_tiles(&new_tile_data);
        self.emu_textures.tile_data = new_tile_data;

        if self.is_tile_viewer_visible() {
            if changed_tiles.is_empty() || changed_tiles.len() > 10 {
                self.emu_textures.update_tile_textures(
                    ctx,
                    &self.config.view_config.palette_rgb_data,
                    None,
                    None,
                );
            } else {
                for tile_idx in changed_tiles {
                    self.emu_textures.update_tile_textures(
                        ctx,
                        &self.config.view_config.palette_rgb_data,
                        None,
                        Some(tile_idx),
                    );
                }
            }
        }
    }

    fn handle_savestate(&mut self, savestate: Box<SaveState>, save_type: SaveType) {
        match save_type {
            SaveType::Manual => {
                util::spawn_save_dialog(
                    Some(&self.async_sender),
                    self.config.user_config.previous_savestate_save_dir.as_ref(),
                    FileType::Savestate,
                    savestate,
                );
            }
            SaveType::Quicksave => {
                self.handle_quicksave(savestate);
            }
            SaveType::Autosave => {
                self.create_auto_save(savestate);
            }
        }
    }

    fn handle_quicksave(&self, savestate: Box<SaveState>) {
        if let Some(rom) = &self.config.user_config.loaded_rom {
            let rom_hash = &rom.data_checksum;
            let prev_name = &self.config.user_config.previous_rom_name;
            if let Some(prev_name) = prev_name {
                let display_name = util::rom_display_name(prev_name, rom_hash);
                let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                let key = storage::quicksave_key(&display_name, &timestamp);

                // Write savestate using storage
                let data = savestate.to_bytes(None);
                #[cfg(not(target_arch = "wasm32"))]
                std::thread::spawn(move || {
                    let _ = storage::write_sync(&key, &data);
                });
                #[cfg(target_arch = "wasm32")]
                wasm_bindgen_futures::spawn_local(async move {
                    let storage = storage::get_storage();
                    let _ = storage.set(&key, data).await;
                });
            }
        }
    }
}
