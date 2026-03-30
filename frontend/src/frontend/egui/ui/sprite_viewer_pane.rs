//! Pattern table viewer pane rendering

use monsoon_core::emulation::ppu_util::SpriteMode;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::textures::EmuTextures;
use crate::frontend::egui::ui::widgets::{
    PainterGridConfig, image_cell_dual_vert_flipped, image_cell_flipped,
};

fn sprite_hover_ui(
    ui: &mut egui::Ui,
    sprite: &monsoon_core::emulation::ppu_util::Sprite,
    mode: SpriteMode,
) {
    ui.label(format!(
        "Tile: {}x{} (Pattern Table {})",
        (sprite.tile & 0xFF) % 16,
        (sprite.tile & 0xFF) / 16,
        sprite.tile >> 8
    ));

    if mode == SpriteMode::TALL {
        ui.label(format!(
            "Bottom Tile: {}x{} (Pattern Table {})",
            (sprite.bottom_tile & 0xFF) % 16,
            (sprite.bottom_tile & 0xFF) / 16,
            sprite.tile >> 8
        ));
    }

    ui.label(format!("Priority: {}", sprite.priority));
    ui.label(format!("Vertical Flip: {}", sprite.v_flip));
    ui.label(format!("Horizontal Flip: {}", sprite.h_flip));
    ui.label(format!("X Position: {}", sprite.x_pos));
    ui.label(format!("Y Position: {}", sprite.y_pos));
    ui.label(format!("Palette Index: {}", sprite.palette));
}

pub fn render_sprite_viewer(ui: &mut egui::Ui, emu_textures: &EmuTextures) {
    if let Some(tile_textures) = &emu_textures.tile_textures
        && let Some(sprite_data) = &emu_textures.sprite_data
    {
        let sprite_mode = sprite_data.mode;
        let sprite_data = sprite_data.sprites;

        let available = ui.available_size();
        let sprite_base_width = 64.0 * (sprite_mode.get_height_mult() as f32);
        let sprite_base_height = 64.0;
        let logical_height = sprite_base_height + 20.0;
        let scale = (available.x / sprite_base_width)
            .min(available.y / logical_height)
            .max(0.1);
        let table_width = sprite_base_width * scale;
        let table_height = sprite_base_height * scale;

        ui.label(format!(
            "Sprites (8x{}x64 at {:.1}x scale)",
            8 * sprite_mode.get_height_mult(),
            scale
        ));

        let (cols, rows) = if sprite_mode == SpriteMode::SMALL {
            (8, 8)
        } else {
            (16, 4)
        };

        let grid_config = PainterGridConfig::rect(table_width, table_height, cols, rows);
        let (parent, _) = ui.allocate_exact_size(grid_config.total_size(), egui::Sense::hover());

        for (i, sprite) in sprite_data.iter().enumerate() {
            let sprite_tile = &tile_textures[sprite.palette as usize][sprite.tile as usize];
            let rect = grid_config.cell_rect(parent.min, i);

            let resp = if sprite_mode == SpriteMode::SMALL {
                image_cell_flipped(
                    ui,
                    rect,
                    sprite_tile.id(),
                    sprite.h_flip,
                    sprite.v_flip,
                    egui::Sense::all(),
                    ("sprite", i),
                )
            } else {
                let lower_tile =
                    &tile_textures[sprite.palette as usize][sprite.bottom_tile as usize];
                image_cell_dual_vert_flipped(
                    ui,
                    rect,
                    sprite_tile.id(),
                    lower_tile.id(),
                    sprite.h_flip,
                    sprite.v_flip,
                    egui::Sense::all(),
                    ("sprite_bottom", i),
                )
            };

            resp.on_hover_ui(|ui| sprite_hover_ui(ui, sprite, sprite_mode));
        }
    } else {
        ui.label("Waiting for sprite data...");
    }
}

pub fn render_soam_viewer(ui: &mut egui::Ui, config: &AppConfig, emu_textures: &EmuTextures) {
    if !config.is_effectively_paused() {
        ui.label("Pause to show soam data");
        return;
    }

    if let Some(tile_textures) = &emu_textures.tile_textures
        && let Some(soam_data) = &emu_textures.soam_data
    {
        let soam_mode = soam_data.mode;
        let soam_sprites = soam_data.sprites;

        let available = ui.available_size();
        let soam_base_width = 16.0;
        let soam_base_height = 32.0 * (soam_mode.get_height_mult() as f32);
        let logical_height = soam_base_height + 20.0;
        let scale = (available.x / soam_base_width)
            .min(available.y / logical_height)
            .max(0.1);
        let soam_table_width = soam_base_width * scale;
        let soam_table_height = soam_base_height * scale;

        ui.label(format!(
            "Soam Sprites (8x{}x8 at {:.1}x scale)",
            8 * soam_mode.get_height_mult(),
            scale
        ));

        let grid_config = PainterGridConfig::rect(soam_table_width, soam_table_height, 2, 4);
        let (parent, _) = ui.allocate_exact_size(grid_config.total_size(), egui::Sense::hover());

        for (i, sprite) in soam_sprites.iter().enumerate() {
            let sprite_tile = &tile_textures[sprite.palette as usize][sprite.tile as usize];
            let rect = grid_config.cell_rect(parent.min, i);

            let resp = if soam_mode == SpriteMode::SMALL {
                image_cell_flipped(
                    ui,
                    rect,
                    sprite_tile.id(),
                    sprite.h_flip,
                    sprite.v_flip,
                    egui::Sense::all(),
                    ("soam_sprite", i),
                )
            } else {
                let lower_tile =
                    &tile_textures[sprite.palette as usize][sprite.bottom_tile as usize];
                image_cell_dual_vert_flipped(
                    ui,
                    rect,
                    sprite_tile.id(),
                    lower_tile.id(),
                    sprite.h_flip,
                    sprite.v_flip,
                    egui::Sense::all(),
                    ("soam_sprite_bottom", i),
                )
            };

            resp.on_hover_ui(|ui| sprite_hover_ui(ui, sprite, soam_mode));
        }
    } else {
        ui.label("Waiting for soam data...");
    }
}
