use egui::Ui;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::fps_counter::FpsCounter;
use crate::frontend::egui::textures::EmuTextures;

/// Add the status bar at the bottom of the window
pub fn add_status_bar(
    ui: &mut Ui,
    fps_counter: &FpsCounter,
    config: &AppConfig,
    emu_textures: &EmuTextures,
) {
    egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("FPS: {:.1}", fps_counter.fps()));
            ui.separator();
            if config.is_effectively_paused() {
                ui.label("Emulator: Paused");
            } else if emu_textures.has_received_frame {
                ui.label("Emulator: Running");
            } else {
                ui.label("Emulator: Initializing");
            }
            if let Some((_, l)) = &config.console_config.loaded_rom {
                ui.separator();
                ui.label(format!("Loaded Rom: {}", l.name));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Ver. {}", monsoon_core::util::VERSION));
            });
        });
    });
}
