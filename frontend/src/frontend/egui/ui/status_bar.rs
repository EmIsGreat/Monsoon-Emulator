use egui::Ui;

use crate::frontend::egui::config::SpeedConfig;
use crate::frontend::egui::fps_counter::FpsCounter;
use crate::frontend::egui::textures::EmuTextures;

/// Add the status bar at the bottom of the window
pub fn add_status_bar(
    ui: &mut Ui,
    fps_counter: &FpsCounter,
    speed_config: &SpeedConfig,
    emu_textures: &EmuTextures,
) {
    egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("FPS: {:.1}", fps_counter.fps()));
            ui.separator();
            if speed_config.is_paused {
                ui.label("Emulator: Paused");
            } else if emu_textures.current_frame.is_some() {
                ui.label("Emulator: Running");
            } else {
                ui.label("Emulator: Initializing");
            }
        });
    });
}
