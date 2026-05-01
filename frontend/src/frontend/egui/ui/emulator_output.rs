//! Emulator output pane rendering

use std::sync::Arc;

use monsoon_core::emulation::ppu_util::{TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH};

use crate::frontend::egui::textures::EmuTextures;
use crate::frontend::egui::wgpu_renderer::{NesWgpuRenderer, WgpuFrameCallback};

/// Render the main emulator output.
///
/// When `wgpu_nes_renderer` is `Some` (wgpu backend active), the pixel buffer
/// is uploaded to the GPU in a `PaintCallback` and displayed via the WGSL
/// palette-lookup shader. Otherwise, the pre-converted CPU texture stored in
/// `emu_textures.frame_texture` is displayed as an egui `Image`.
pub fn render_emulator_output(
    ui: &mut egui::Ui,
    emu_textures: &EmuTextures,
    wgpu_nes_renderer: Option<&Arc<NesWgpuRenderer>>,
    is_paused: bool,
) {
    if !emu_textures.has_received_frame {
        ui.label("Waiting for first frame...");
        return;
    }

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

    let response = if wgpu_nes_renderer.is_some() {
        // GPU path: allocate the rect, then add a PaintCallback that uploads
        // the front-buffer and runs the WGSL palette shader inside it.
        let (response, painter) = ui.allocate_painter(image_size, egui::Sense::hover());
        let rect = response.rect;

        let callback = egui_wgpu::Callback::new_paint_callback(
            rect,
            WgpuFrameCallback {
                frame: Arc::new(emu_textures.front_buffer.clone()),
            },
        );
        painter.add(callback);
        response
    } else if let Some(ref texture) = emu_textures.frame_texture {
        // CPU path: display the pre-built egui texture.
        ui.add(egui::Image::new((texture.id(), image_size)))
    } else {
        ui.label("Waiting for first frame...");
        return;
    };

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
}
