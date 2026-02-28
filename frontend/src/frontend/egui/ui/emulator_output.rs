//! Emulator output pane rendering

use monsoon_core::emulation::ppu_util::{TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH};

use crate::frontend::egui::textures::EmuTextures;

/// Render the main emulator output
pub fn render_emulator_output(ui: &mut egui::Ui, emu_textures: &EmuTextures) {
    if let Some(ref texture) = emu_textures.frame_texture {
        let available = ui.available_size();

        let scale =
            (available.x / TOTAL_OUTPUT_WIDTH as f32).min(available.y / TOTAL_OUTPUT_HEIGHT as f32);

        let display_width = TOTAL_OUTPUT_WIDTH as f32 * scale;
        let display_height = TOTAL_OUTPUT_HEIGHT as f32 * scale;

        ui.label(format!(
            "{}x{} at {:.1}x scale",
            TOTAL_OUTPUT_WIDTH, TOTAL_OUTPUT_HEIGHT, scale
        ));

        ui.image((texture.id(), egui::vec2(display_width, display_height)));
    } else {
        ui.label("Waiting for first frame...");
    }
}
