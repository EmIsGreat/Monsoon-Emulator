//! Options pane rendering

use crossbeam_channel::Sender;
use monsoon_core::emulation::cpu::SHAMode;
use monsoon_core::emulation::nes::CpuAlignment;
use monsoon_core::emulation::screen_renderer::ScreenRenderer;

use crate::frontend::egui::config::{AppConfig, AppSpeed, DebugSpeed};
use crate::frontend::egui::ui::widgets::NumInput;
use crate::frontend::messages::AsyncFrontendMessage;
use crate::get_all_renderers;

/// Render the options panel
pub fn render_options(
    ui: &mut egui::Ui,
    config: &mut AppConfig,
    sender: &Sender<AsyncFrontendMessage>,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        render_speed_settings(ui, config);
        render_renderer_settings(ui, config);
        render_debug_overlay_settings(ui, config);
        render_rom_loading_settings(ui, config);
        render_emulation_config_settings(ui, config, sender);
    });
}

fn render_rom_loading_settings(ui: &mut egui::Ui, config: &mut AppConfig) {
    ui.collapsing("Rom Loading", |ui| {
        ui.checkbox(&mut config.user_config.use_rom_db, "Use ROM header DB")
            .on_hover_text("Use the bundled ROM DB for enhanced compatibility")
    });
}

/// Render renderer selection section
fn render_renderer_settings(ui: &mut egui::Ui, config: &mut AppConfig) {
    ui.collapsing("Renderer", |ui| {
        // Display the renderer type name
        ui.label(format!(
            "Current Renderer: {}",
            config.view_config.renderer.get_display_name()
        ));

        ui.separator();

        // Renderer selection dropdown
        ui.label("Select Renderer:");
        let current_id = config.view_config.renderer.get_display_name().to_string();
        egui::ComboBox::from_id_salt("renderer_selector")
            .selected_text(config.view_config.renderer.get_display_name())
            .show_ui(ui, |ui| {
                for variant in get_all_renderers() {
                    let selected = variant.key == current_id;
                    if ui
                        .selectable_label(selected, variant.display_name)
                        .clicked()
                    {
                        // Transfer the current palette to the new renderer
                        // Note: This copies the palette (~1.5KB), but this is
                        // an infrequent UI operation
                        let palette = config.view_config.palette_rgb_data;
                        let mut renderer: Box<dyn ScreenRenderer> = (variant.factory)();
                        renderer.set_palette(palette);
                        config.view_config.renderer = renderer;
                    }
                }
            });

        ui.separator();

        // Show current palette
        ui.label(format!(
            "Current palette: {}",
            config
                .user_config
                .previous_palette
                .as_ref()
                .map_or("Bundled 2C02-G Palette".to_string(), |k| k
                    .get_leaf_name()
                    .clone())
        ));
        ui.small("Use the Palette viewer to load custom palette files.");
    });
}

/// Render speed settings section
fn render_speed_settings(ui: &mut egui::Ui, config: &mut AppConfig) {
    ui.collapsing("Speed", |ui| {
        ui.label("Emulation Speed")
            .on_hover_text("Sets the speed at which the emulation runs");
        ui.radio_value(
            &mut config.speed_config.app_speed,
            AppSpeed::DefaultSpeed,
            "Default (60fps)",
        );
        ui.radio_value(
            &mut config.speed_config.app_speed,
            AppSpeed::Custom,
            "Custom",
        );
        ui.radio_value(
            &mut config.speed_config.app_speed,
            AppSpeed::Uncapped,
            "Uncapped",
        );

        if config.speed_config.app_speed == AppSpeed::Custom {
            ui.add(
                egui::Slider::new(&mut config.speed_config.custom_speed, 0..=500)
                    .text("Speed")
                    .suffix("%")
                    .fixed_decimals(0)
                    .logarithmic(true),
            );
        }
        ui.separator();
        ui.label("Debug Viewer Speed")
            .on_hover_text("Sets the speed at which the debug views update");
        ui.radio_value(
            &mut config.speed_config.debug_speed,
            DebugSpeed::DefaultSpeed,
            "10fps",
        );
        ui.radio_value(
            &mut config.speed_config.debug_speed,
            DebugSpeed::Custom,
            "Custom",
        );
        ui.radio_value(
            &mut config.speed_config.debug_speed,
            DebugSpeed::InStep,
            "Realtime",
        );
        if config.speed_config.debug_speed == DebugSpeed::Custom {
            ui.add(
                egui::Slider::new(&mut config.speed_config.debug_custom_speed, 0..=100)
                    .text("Debug Speed")
                    .suffix("%")
                    .fixed_decimals(0)
                    .logarithmic(true),
            )
            .on_hover_text("% of main view fps");
        }
    });
}

/// Render debug overlay toggles for the main emulator output.
fn render_debug_overlay_settings(ui: &mut egui::Ui, config: &mut AppConfig) {
    ui.collapsing("Debug Overlays", |ui| {
        ui.checkbox(
            &mut config.view_config.debug_overlays.show_tile_grid,
            "Tile grid (8x8)",
        );
        ui.checkbox(
            &mut config.view_config.debug_overlays.show_scanline_dot,
            "Scanline/dot indicator (paused)",
        );
    });
}

fn render_emulation_config_settings(
    ui: &mut egui::Ui,
    config: &mut AppConfig,
    sender: &Sender<AsyncFrontendMessage>,
) {
    ui.collapsing("Emulation Settings", |ui| {
        ui.label("CPU-PPU Alignment");
        let prev_align = config.console_config.nes_config.alignment;

        ui.horizontal(|ui| {
            ui.radio_value(
                &mut config.console_config.nes_config.alignment,
                CpuAlignment::Offset0,
                "0",
            );
            ui.radio_value(
                &mut config.console_config.nes_config.alignment,
                CpuAlignment::Offset1,
                "1",
            );
            ui.radio_value(
                &mut config.console_config.nes_config.alignment,
                CpuAlignment::Offset2,
                "2",
            );
            ui.radio_value(
                &mut config.console_config.nes_config.alignment,
                CpuAlignment::Offset3,
                "3",
            );
        });

        if prev_align != config.console_config.nes_config.alignment {
            let _ = sender.send(AsyncFrontendMessage::ConfigChanged);
        }

        ui.separator();
        ui.label("SHA Mode");

        let prev_mode = config.console_config.nes_config.sha_mode;

        ui.horizontal(|ui| {
            ui.radio_value(
                &mut config.console_config.nes_config.sha_mode,
                SHAMode::Mode1,
                "1",
            );
            ui.radio_value(
                &mut config.console_config.nes_config.sha_mode,
                SHAMode::Mode2,
                "2",
            );
            ui.radio_value(
                &mut config.console_config.nes_config.sha_mode,
                SHAMode::Mode3,
                "3",
            );
            ui.radio_value(
                &mut config.console_config.nes_config.sha_mode,
                SHAMode::Mode4,
                "4",
            );
        });

        if prev_mode != config.console_config.nes_config.sha_mode {
            let _ = sender.send(AsyncFrontendMessage::ConfigChanged);
        }

        if config.console_config.nes_config.sha_mode == SHAMode::Mode3 {
            ui.label("SHA Address Magic Byte");
            let prev_address_magic = config.console_config.nes_config.sha_mode3_address_magic;

            let input = NumInput::new(
                ui,
                "sha_mode3_address_magic_byte_input",
                &mut config.console_config.nes_config.sha_mode3_address_magic,
            )
            .prefix("$".to_string())
            .radix(16);

            ui.add(input);

            if prev_address_magic != config.console_config.nes_config.sha_mode3_address_magic {
                let _ = sender.send(AsyncFrontendMessage::ConfigChanged);
            }
        }

        ui.label("SHA Magic Byte");
        let prev_magic = config.console_config.nes_config.sha_magic;

        let input = NumInput::new(
            ui,
            "sha_magic_byte_input",
            &mut config.console_config.nes_config.sha_magic,
        )
        .prefix("$".to_string())
        .radix(16);

        ui.add(input);

        if prev_magic != config.console_config.nes_config.sha_magic {
            let _ = sender.send(AsyncFrontendMessage::ConfigChanged);
        }
    });
}
