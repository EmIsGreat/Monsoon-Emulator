//! Pattern table viewer pane rendering

use monsoon_core::emulation::ppu_util::SpriteMode;

use crate::frontend::egui::textures::EmuTextures;
use crate::frontend::egui::ui::widgets::{PainterGridConfig, image_cell_flipped, image_cell_dual_vert_flipped};

/// Render both pattern tables side by side
pub fn render_sprite_viewer(ui: &mut egui::Ui, emu_textures: &EmuTextures) {
    if let Some(tile_textures) = &emu_textures.tile_textures
        && let Some(sprite_data) = &emu_textures.sprite_data
    {
        let sprite_mode = sprite_data.mode;
        let sprite_data = sprite_data.sprites;

        let base_table_width = 64.0;
        let base_table_height = 64.0 * (sprite_mode.get_height_mult() as f32);

        let available = ui.available_size();

        let logical_width = base_table_width + 8.0;
        let logical_height = base_table_height + 20.0; // +20 for label

        // Scale to fit both width and height
        let scale = (available.x / logical_width).min(available.y / logical_height);
        let table_width = base_table_width * scale;
        let table_height = base_table_height * scale;

        // Palette selector and label in horizontal layout
        ui.horizontal(|ui| {
            ui.label(format!(
                "Sprites (8x{}x64 at {:.1}x scale)",
                8 * sprite_mode.get_height_mult(),
                scale
            ));
        });

        ui.horizontal_top(|ui| {
            let grid_config = PainterGridConfig::rect(table_width, table_height, 8, 8);
            let (parent, _) =
                ui.allocate_exact_size(grid_config.total_size(), egui::Sense::hover());

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

                resp.on_hover_ui(|ui| {
                    ui.label(format!(
                        "Tile: {}x{} (Pattern Table {})",
                        (sprite.tile & 0xFF) % 16,
                        (sprite.tile & 0xFF) / 16,
                        sprite.tile >> 8
                    ));

                    if sprite_mode == SpriteMode::TALL {
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
                });
            }
        });
    } else {
        ui.label("Waiting for sprite data...");
    }
}
