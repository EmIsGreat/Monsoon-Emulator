//! Emulator output pane rendering

use monsoon_core::emulation::ppu_util::{TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH};

use crate::frontend::egui::textures::EmuTextures;

/// Render the main emulator output
pub fn render_emulator_output(ui: &mut egui::Ui, emu_textures: &EmuTextures, is_paused: bool) {
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

        let image_size = egui::vec2(display_width, display_height);
        let response = ui.add(egui::Image::new((texture.id(), image_size)));

        if is_paused {
            let overlay_rect = response.rect;
            let painter = ui.painter();
            painter.rect_filled(
                overlay_rect,
                6.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120),
            );
            painter.text(
                overlay_rect.center(),
                egui::Align2::CENTER_CENTER,
                "⏸ EMULATION PAUSED",
                egui::TextStyle::Heading.resolve(ui.style()),
                egui::Color32::WHITE,
            );
        }
    } else {
        ui.label("Waiting for first frame...");
    }
}
