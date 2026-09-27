//! Emulator message handler for backend communication.
//!
//! This module handles messages from the emulator backend, including
//! frame updates, debug data, and savestate operations.

use egui::{Context, ViewportCommand};
use monsoon_core::emulation::ppu_util::{EmulatorFetchable, PaletteData, TILE_COUNT, TileData};

use crate::frontend::egui_frontend::EguiApp;
use crate::messages::EmulatorMessage;

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
            EmulatorMessage::FrameReady => {
                // Swap the back buffer (in ChannelEmulator) with the front
                // buffer (in EmuTextures). Both are separate struct fields so
                // Rust allows simultaneous mutable borrows. This is the
                // zero-copy "back ↔ front" step of the triple-buffer pipeline.
                std::mem::swap(
                    &mut self.channel_emu.back_buffer,
                    &mut self.emu_textures.front_buffer,
                );
                self.emu_textures.has_received_frame = true;
                self.fps_counter.update();

                // When the wgpu renderer is active, the GPU upload happens
                // lazily in WgpuFrameCallback::prepare() during the paint
                // phase. Skip the CPU-side texture upload in that case.
                if self.wgpu_nes_renderer.is_none() {
                    self.emu_textures
                        .update_emulator_texture(ctx, &mut self.config.view_config.renderer);
                }
            }
            EmulatorMessage::DebugData(data) => {
                self.handle_debug_data(ctx, data);
            }
            EmulatorMessage::Stopped => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            EmulatorMessage::RomLoaded(rom) => {
                self.config.console_config.loaded_rom = *rom;
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
            EmulatorFetchable::Sprites(s) => {
                self.emu_textures.sprite_data = s;
            }
            EmulatorFetchable::SoamSprites(s) => {
                self.emu_textures.soam_data = s;
            }
            EmulatorFetchable::Registers(r) => {
                self.emu_textures.register_data = r;
            }
        }
    }

    fn handle_palette_data(&mut self, ctx: &Context, new_palette_data: Option<Box<PaletteData>>) {
        // Only rebuild textures if palette data actually changed and a tile
        // viewer is visible
        if self.emu_textures.palette_data != new_palette_data {
            let changed_palettes = self.detect_changed_palettes(new_palette_data.as_deref());
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
        new_tile_data: Option<Box<[TileData; usize::from(TILE_COUNT)]>>,
    ) {
        let changed_tiles = self.detect_changed_tiles(new_tile_data.as_deref());
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
}
