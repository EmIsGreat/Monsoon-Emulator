//! Nametable viewer pane rendering

use crate::frontend::egui::textures::EmuTextures;

/// Render all 4 nametables in a 2x2 grid layout
pub fn render_nametable(ui: &mut egui::Ui, emu_textures: &EmuTextures) {
    if let Some(ref data) = emu_textures.nametable_data
        && let Some(ref textures) = emu_textures.tile_textures
    {
        let available = ui.available_size();
        // Each nametable is 32x30 tiles, we show 2x2 nametables
        let base_size = 8.0;
        let tile_cols: u8 = 32;
        let tile_rows: u8 = 30;
        // 2 nametables side by side, 2 stacked vertically
        let logical_width = (f32::from(tile_cols) * base_size) * 2.0 + 4.0; // +4 for spacing
        let logical_height = (f32::from(tile_rows) * base_size) * 2.0 + 4.0 + 20.0; // +4 spacing,
        // +20 for label
        // Scale to fit both width and height
        let scale = (available.x / logical_width).min(available.y / logical_height);
        let tex_size = egui::vec2(base_size, base_size) * scale;

        ui.label(format!("Nametables (256x240 x4 at {scale:.1}x scale)"));

        // Render 4 nametables in a 2x2 grid
        egui::Grid::new("nametables_container")
            .num_columns(2)
            .spacing(egui::vec2(4.0, 4.0))
            .show(ui, |ui| {
                for nametable_index in 0..4 {
                    if nametable_index < data.tiles.len() {
                        let nametable = &data.tiles[nametable_index];

                        egui::Grid::new(format!("nametable_{nametable_index}"))
                            .num_columns(32)
                            .min_row_height(scale * base_size)
                            .min_col_width(scale * base_size)
                            .max_col_width(scale * base_size)
                            .spacing(egui::vec2(0.0, 0.0))
                            .show(ui, |ui| {
                                for (i, tile) in nametable.iter().enumerate() {
                                    // Calculate tile position in the 32x30 nametable
                                    let tile_col = i % 32;
                                    let tile_row = i / 32;

                                    // Each attribute byte covers a 4x4 tile area (32x32 pixels)
                                    // Attribute table is 8 bytes wide (8 * 4 = 32 tiles)
                                    let attr_col = tile_col / 4;
                                    let attr_row = tile_row / 4;
                                    let attr_table_byte =
                                        data.palettes[nametable_index][attr_row * 8 + attr_col];

                                    // Each attribute byte contains 4 palette indices (2 bits each)
                                    // for the four 2x2 tile quadrants within the 4x4 area:
                                    //   bits 0-1: top-left     bits 2-3: top-right
                                    //   bits 4-5: bottom-left  bits 6-7: bottom-right
                                    // Use bit 1 of tile position to determine quadrant
                                    let shift = ((tile_row & 2) << 1) | (tile_col & 2);

                                    let palette = (attr_table_byte >> shift) & 0b11;
                                    let texture = &textures[palette as usize][*tile as usize];
                                    ui.image((texture.id(), tex_size));

                                    if (i + 1) % 32 == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    }

                    // End row after every 2 nametables (0,1 on first row, 2,3 on second)
                    if nametable_index % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    } else {
        ui.label("Waiting for nametable data...");
    }
}
