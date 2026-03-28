use crossbeam_channel::Sender;
use monsoon_core::emulation::palette_util::{RgbColor, parse_palette_from_bytes};
use monsoon_core::emulation::ppu_util::PALETTE_RAM_START_ADDRESS;
use monsoon_core::util::Hashable;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::textures::EmuTextures;
use crate::frontend::egui::ui::widgets::{PainterGridConfig, color_cell_rgb};
use crate::frontend::messages::{AsyncFrontendMessage, LoadedPalette};
use crate::frontend::storage::{StorageCategory, StorageKey};
use crate::frontend::util::{self, FileType, spawn_palette_picker, spawn_save_dialog};

pub fn render_palettes(
    ui: &mut egui::Ui,
    config: &mut AppConfig,
    emu_textures: &EmuTextures,
    async_sender: &Sender<AsyncFrontendMessage>,
) {
    let old_palette = config.view_config.palette_rgb_data;
    let full_width = ui.available_width();
    let single_color_width = 80f32.min(full_width / 4.0);
    let single_color_height = 20.0;

    // Render the 8 active palettes (4 background + 4 sprite)
    if let Some(palette_data) = &emu_textures.palette_data {
        for (i, palette) in palette_data.colors.iter().enumerate() {
            if i < 4 {
                ui.label(format!("Background Palette {}", i + 1));
            } else {
                ui.label(format!("Sprite Palette {}", i - 3));
            };

            let grid_config =
                PainterGridConfig::rect(single_color_width * 4.0, single_color_height, 4, 1);
            let (parent, _) =
                ui.allocate_exact_size(grid_config.total_size(), egui::Sense::hover());

            for (j, color) in palette.iter().enumerate() {
                let rgb_color = config.view_config.palette_rgb_data.colors[0][*color as usize];
                let rect = grid_config.cell_rect(parent.min, j);
                let response =
                    color_cell_rgb(ui, rect, rgb_color, egui::Sense::all(), ("palette", i, j));

                let address = PALETTE_RAM_START_ADDRESS as usize | (j + (i * 4));
                let mut new_color = *color;

                egui::Popup::context_menu(&response)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.add(egui::Slider::new(&mut new_color, 1..=64).text("Palette Index"));
                    });

                if new_color != *color {
                    let _ = async_sender.send(AsyncFrontendMessage::WritePpuPalette {
                        address: address as u16,
                        value: new_color,
                    });
                }

                response.on_hover_ui(|ui| {
                    ui.label(format!("Global palette index: ${color}"));
                    ui.label(format!("Address: ${:0X}", address));
                    ui.label(format!(
                        "Palette RGB mapping: #{:02X}{:02X}{:02X}",
                        rgb_color.r, rgb_color.g, rgb_color.b
                    ));
                });
            }
        }
    }

    // File operations menu
    ui.separator();
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Load Palette").clicked() {
                spawn_palette_picker(
                    async_sender,
                    config.user_config.previous_palette_load_dir.as_ref(),
                );
            }

            if ui.button("Save Palette").clicked() {
                spawn_save_dialog(
                    Some(async_sender),
                    config.user_config.previous_palette_save_dir.as_ref(),
                    FileType::Palette,
                    Box::new(config.view_config.palette_rgb_data),
                );
            }

            if ui.button("Reset Palette").clicked() {
                let sender = async_sender.clone();

                util::spawn_async(async move {
                    // Reset to default palette
                    let palette = parse_palette_from_bytes(&[]);
                    let _ = sender.send(AsyncFrontendMessage::PaletteLoaded(LoadedPalette {
                        palette,
                        directory: StorageKey {
                            category: StorageCategory::Cache,
                            sub_path: "default_palettes/".to_string(),
                        },
                    }));
                });
            }
        })
    });

    // Render the full 64-color palette editor
    let color_width = 40f32.min(full_width / 8.0);
    let grid_config = PainterGridConfig::square(color_width * 8.0, 8);
    let (parent, _) = ui.allocate_exact_size(grid_config.total_size(), egui::Sense::hover());

    for (i, color) in config.view_config.palette_rgb_data.colors[0]
        .clone()
        .iter()
        .enumerate()
    {
        let rect = grid_config.cell_rect(parent.min, i);
        let response = color_cell_rgb(ui, rect, *color, egui::Sense::all(), ("rgb_palette", i));

        // Convert RgbColor to Color32 for the color picker
        let mut picked_color = egui::Color32::from_rgb(color.r, color.g, color.b);
        egui::Popup::context_menu(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                egui::color_picker::color_picker_color32(
                    ui,
                    &mut picked_color,
                    egui::color_picker::Alpha::Opaque,
                );
            });

        // Convert Color32 back to RgbColor
        let [r, g, b, _] = picked_color.to_array();
        config.view_config.palette_rgb_data.colors[0][i] = RgbColor::new(r, g, b);

        response.on_hover_ui(|ui| {
            ui.label(format!("Index: {i}"));
            ui.label(format!("RGB: ({}, {}, {})", color.r, color.g, color.b));
        });
    }

    // Send palette update if changed
    if old_palette.hash() != config.view_config.palette_rgb_data.hash() {
        let _ = async_sender.send(AsyncFrontendMessage::SetPalette(
            config.view_config.palette_rgb_data,
        ));
    }
}
