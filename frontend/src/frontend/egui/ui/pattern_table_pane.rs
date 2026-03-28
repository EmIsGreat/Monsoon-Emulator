//! Pattern table viewer pane rendering

use crossbeam_channel::Sender;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::textures::EmuTextures;
use crate::frontend::egui::ui::draw_pattern_table;
use crate::frontend::messages::AsyncFrontendMessage;

/// Render both pattern tables side by side
pub fn render_pattern_table(
    ui: &mut egui::Ui,
    config: &mut AppConfig,
    emu_textures: &EmuTextures,
    async_sender: &Sender<AsyncFrontendMessage>,
) {
    if let Some(tile_textures) = &emu_textures.tile_textures
        && let Some(palettes) = &emu_textures.palette_data
        && let Some(pattern_data) = &emu_textures.tile_data
    {
        let available = ui.available_size();
        // Each pattern table is 128x128 pixels (16x16 tiles * 8 pixels each)
        // We show 2 side by side with spacing
        // Fixed (non-scaling) UI elements: spacing between tables, label + palette
        // selector
        let fixed_width = ui.spacing().item_spacing.x * 3.0;
        let fixed_height = 40.0; // label and palette selector
        // Scale to fit: subtract fixed elements from available space before dividing
        let scale = ((available.x - fixed_width) / (128.0 * 2.0))
            .min((available.y - fixed_height) / 128.0)
            .max(0.1);
        let table_size = 128.0 * scale;

        // Palette selector and label in horizontal layout
        ui.horizontal(|ui| {
            ui.label(format!("Pattern Tables (128x128x2 at {:.1}x scale)", scale));
            ui.separator();
            ui.label("Debug Palette:");
            let palette_label = if config.user_config.debug_active_palette < 4 {
                format!("BG Palette {}", config.user_config.debug_active_palette + 1)
            } else {
                format!(
                    "Sprite Palette {}",
                    config.user_config.debug_active_palette - 3
                )
            };
            egui::ComboBox::from_id_salt("debug_palette_selector")
                .selected_text(palette_label)
                .show_ui(ui, |ui| {
                    for i in 0..8 {
                        let label = if i < 4 {
                            format!("BG Palette {}", i + 1)
                        } else {
                            format!("Sprite Palette {}", i - 3)
                        };
                        ui.selectable_value(&mut config.user_config.debug_active_palette, i, label);
                    }
                });
        });

        let selected_palette = palettes.colors[config.user_config.debug_active_palette];
        let transformed_palette = selected_palette
            .map(|color_index| config.view_config.palette_rgb_data.colors[0][color_index as usize]);

        ui.horizontal_top(|ui| {
            draw_pattern_table(
                ui,
                table_size,
                &tile_textures[config.user_config.debug_active_palette][..256],
                transformed_palette,
                &pattern_data[..256],
                async_sender,
                config,
            );

            ui.separator();
            draw_pattern_table(
                ui,
                table_size,
                &tile_textures[config.user_config.debug_active_palette][256..],
                transformed_palette,
                &pattern_data[256..],
                async_sender,
                config,
            );
        });
    } else {
        ui.label("Waiting for pattern table data...");
    }
}
