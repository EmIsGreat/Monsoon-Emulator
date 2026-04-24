use crossbeam_channel::Sender;
use eframe::epaint::TextureHandle;
use egui::Ui;
use monsoon_core::emulation::palette_util::RgbColor;
use monsoon_core::emulation::ppu_util::TileData;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::ui::widgets::{PainterGridConfig, image_cell};
use crate::frontend::messages::AsyncFrontendMessage;
use crate::frontend::util::color_radio;

/// Draw a pattern table (left or right) in the UI
pub fn draw_pattern_table(
    ui: &mut Ui,
    width: f32,
    emu_textures: &[TextureHandle],
    palette: [RgbColor; 4],
    pattern_data: &[TileData],
    async_sender: &Sender<AsyncFrontendMessage>,
    config: &mut AppConfig,
) {
    let grid_config = PainterGridConfig::square(width, 16);
    let (parent, _) = ui.allocate_exact_size(grid_config.total_size(), egui::Sense::hover());

    for (i, tex) in emu_textures.iter().enumerate() {
        let rect = grid_config.cell_rect(parent.min, i);
        let response = image_cell(
            ui,
            rect,
            tex.id(),
            egui::Sense::all(),
            ("tile", i, parent.to_string()),
        );

        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("popup_painter"),
        ));

        egui::Popup::context_menu(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.heading("Edit Pattern");

                let tile_data = pattern_data[i];
                ui.label(format!("Rom address: ${:0X}", tile_data.address));

                ui.label("Pattern:");

                ui.horizontal(|ui| {
                    color_radio(
                        ui,
                        &mut config.user_config.pattern_edit_color,
                        0,
                        egui::Color32::from_rgb(palette[0].r, palette[0].g, palette[0].b),
                    );
                    color_radio(
                        ui,
                        &mut config.user_config.pattern_edit_color,
                        1,
                        egui::Color32::from_rgb(palette[1].r, palette[1].g, palette[1].b),
                    );
                    color_radio(
                        ui,
                        &mut config.user_config.pattern_edit_color,
                        2,
                        egui::Color32::from_rgb(palette[2].r, palette[2].g, palette[2].b),
                    );
                    color_radio(
                        ui,
                        &mut config.user_config.pattern_edit_color,
                        3,
                        egui::Color32::from_rgb(palette[3].r, palette[3].g, palette[3].b),
                    );
                });

                // Tile pixel editor grid (8x8 pixels, each 4x4 in size = 32x32 total)
                let pixel_size = 4.0 * 8.0;
                let pixel_grid = PainterGridConfig::square(pixel_size * 8.0, 8);
                let (pixel_parent, _) =
                    ui.allocate_exact_size(pixel_grid.total_size(), egui::Sense::click_and_drag());

                for index in 0..64 {
                    let lo = (tile_data.plane_0 >> (63 - index) & 1) as u8;
                    let hi = (tile_data.plane_1 >> (63 - index) & 1) as u8;
                    let color_id = hi << 1 | lo;
                    let color = palette[color_id as usize];

                    let pixel_rect = pixel_grid.cell_rect(pixel_parent.min, index);
                    let response = ui.interact(
                        pixel_rect,
                        ui.id().with(("pixel", index)),
                        egui::Sense::click_and_drag(),
                    );

                    painter.rect_filled(
                        pixel_rect,
                        0.0,
                        egui::Color32::from_rgb(color.r, color.g, color.b),
                    );

                    // Support both click and drag for editing pixels
                    // Check if clicked OR if pointer is within this pixel's rect while primary
                    // button is down
                    let pointer_in_rect = ui.ctx().input(|i| {
                        i.pointer
                            .interact_pos()
                            .is_some_and(|pos| pixel_rect.contains(pos))
                    });
                    let primary_down = ui.input(|i| i.pointer.primary_down());
                    let secondary_down = ui.input(|i| i.pointer.secondary_down());
                    if response.clicked() || (pointer_in_rect && primary_down) {
                        handle_pixel_edit(&tile_data, index, config, async_sender, false);
                    }

                    if response.clicked_by(egui::PointerButton::Secondary)
                        || (pointer_in_rect && secondary_down)
                    {
                        handle_pixel_edit(&tile_data, index, config, async_sender, true);
                    }

                    if response.hovered() || pointer_in_rect {
                        painter.rect_stroke(
                            pixel_rect,
                            0.0,
                            egui::Stroke::new(3f32, egui::Color32::WHITE),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
            });
    }
}

/// Handle editing a single pixel in a tile pattern
fn handle_pixel_edit(
    tile_data: &TileData,
    index: usize,
    config: &AppConfig,
    async_sender: &Sender<AsyncFrontendMessage>,
    clear: bool,
) {
    let new_color = if !clear {
        config.user_config.pattern_edit_color
    } else {
        0
    };

    let new_lo = new_color & 1;
    let new_hi = (new_color >> 1) & 1;

    let u64_bit_pos = 63usize.wrapping_sub(index) & 63;
    let ppu_row = 7 - (u64_bit_pos / 8);
    let ppu_bit = u64_bit_pos % 8;

    let byte_shift = (7 - ppu_row) * 8;
    let old_byte_0 = ((tile_data.plane_0 >> byte_shift) & 0xFF) as u8;
    let old_byte_1 = ((tile_data.plane_1 >> byte_shift) & 0xFF) as u8;

    let new_byte_0 = (old_byte_0 & !(1 << ppu_bit)) | (new_lo << ppu_bit);
    let new_byte_1 = (old_byte_1 & !(1 << ppu_bit)) | (new_hi << ppu_bit);

    if old_byte_0 != new_byte_0 || old_byte_1 != new_byte_1 {
        let addr_0 = tile_data.address + ppu_row as u16;
        let addr_1 = tile_data.address + ppu_row as u16 + 8;

        let _ = async_sender.send(AsyncFrontendMessage::WritePpuPattern {
            addr_0,
            value_0: new_byte_0,
            addr_1,
            value_1: new_byte_1,
        });
    }
}
