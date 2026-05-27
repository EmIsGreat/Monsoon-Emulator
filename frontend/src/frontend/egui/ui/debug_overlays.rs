use eframe::egui;
use monsoon_core::emulation::ppu_util::{
    RegisterMap, RegisterValue, TILE_SIZE, TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH,
};

use crate::frontend::egui::config::DebugOverlayConfig;
use crate::frontend::egui::textures::EmuTextures;

#[derive(Clone, Copy)]
struct PpuPosition {
    scanline: u16,
    dot: u16,
}

struct OverlayContext<'a> {
    rect: egui::Rect,
    painter: &'a egui::Painter,
    pixel_size: egui::Vec2,
    is_paused: bool,
    ppu_position: Option<PpuPosition>,
    pixels_per_point: f32,
}

trait DebugOverlay {
    fn is_enabled(&self, config: &DebugOverlayConfig) -> bool;
    fn draw(&self, ctx: &OverlayContext);
}

struct TileGridOverlay;

impl DebugOverlay for TileGridOverlay {
    fn is_enabled(&self, config: &DebugOverlayConfig) -> bool { config.show_tile_grid }

    fn draw(&self, ctx: &OverlayContext) {
        let stroke_width = 1.0 / ctx.pixels_per_point;
        let stroke = egui::Stroke::new(
            stroke_width,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 48),
        );

        let cols = TOTAL_OUTPUT_WIDTH / TILE_SIZE;
        let rows = TOTAL_OUTPUT_HEIGHT / TILE_SIZE;

        for col in 1..cols {
            let x = ctx.rect.left() + (col as f32 * TILE_SIZE as f32 * ctx.pixel_size.x);
            ctx.painter.line_segment(
                [egui::pos2(x, ctx.rect.top()), egui::pos2(x, ctx.rect.bottom())],
                stroke,
            );
        }

        for row in 1..rows {
            let y = ctx.rect.top() + (row as f32 * TILE_SIZE as f32 * ctx.pixel_size.y);
            ctx.painter.line_segment(
                [egui::pos2(ctx.rect.left(), y), egui::pos2(ctx.rect.right(), y)],
                stroke,
            );
        }
    }
}

struct ScanlineDotOverlay;

impl DebugOverlay for ScanlineDotOverlay {
    fn is_enabled(&self, config: &DebugOverlayConfig) -> bool { config.show_scanline_dot }

    fn draw(&self, ctx: &OverlayContext) {
        if !ctx.is_paused {
            return;
        }

        let Some(position) = ctx.ppu_position else {
            return;
        };

        let stroke_width = 1.0 / ctx.pixels_per_point;
        let dot_stroke = egui::Stroke::new(
            stroke_width,
            egui::Color32::from_rgba_unmultiplied(0, 110, 0, 210),
        );
        let dot_color = egui::Color32::from_rgba_unmultiplied(0, 110, 0, 170);
        let scanline_color = egui::Color32::from_rgba_unmultiplied(120, 220, 140, 90);

        if position.scanline < TOTAL_OUTPUT_HEIGHT as u16 {
            let y = ctx.rect.top() + (position.scanline as f32 * ctx.pixel_size.y);
            let scanline_rect = egui::Rect::from_min_size(
                egui::pos2(ctx.rect.left(), y),
                egui::vec2(ctx.rect.width(), ctx.pixel_size.y),
            );
            ctx.painter.rect_filled(scanline_rect, 0.0, scanline_color);
        }

        if position.dot < TOTAL_OUTPUT_WIDTH as u16 {
            let x = ctx.rect.left() + (position.dot as f32 * ctx.pixel_size.x);
            ctx.painter.line_segment(
                [egui::pos2(x, ctx.rect.top()), egui::pos2(x, ctx.rect.bottom())],
                dot_stroke,
            );
        }

        if position.dot < TOTAL_OUTPUT_WIDTH as u16 && position.scanline < TOTAL_OUTPUT_HEIGHT as u16
        {
            let min = egui::pos2(
                ctx.rect.left() + (position.dot as f32 * ctx.pixel_size.x),
                ctx.rect.top() + (position.scanline as f32 * ctx.pixel_size.y),
            );
            let dot_rect = egui::Rect::from_min_size(min, ctx.pixel_size);
            ctx.painter.rect_filled(dot_rect, 0.0, dot_color);
        }
    }
}

pub fn render_debug_overlays(
    ui: &egui::Ui,
    rect: egui::Rect,
    config: &DebugOverlayConfig,
    emu_textures: &EmuTextures,
    is_paused: bool,
) {
    if !config.show_tile_grid && !config.show_scanline_dot {
        return;
    }

    let painter = ui.painter_at(rect);
    let pixel_size = egui::vec2(
        rect.width() / TOTAL_OUTPUT_WIDTH as f32,
        rect.height() / TOTAL_OUTPUT_HEIGHT as f32,
    );
    let ppu_position =
        emu_textures
            .register_data
            .as_ref()
            .and_then(|data| ppu_position_from_registers(&data.ppu));

    let ctx = OverlayContext {
        rect,
        painter: &painter,
        pixel_size,
        is_paused,
        ppu_position,
        pixels_per_point: ui.ctx().pixels_per_point(),
    };

    let tile_grid = TileGridOverlay;
    let scanline = ScanlineDotOverlay;
    for overlay in [&tile_grid as &dyn DebugOverlay, &scanline] {
        if overlay.is_enabled(config) {
            overlay.draw(&ctx);
        }
    }
}

fn ppu_position_from_registers(registers: &RegisterMap) -> Option<PpuPosition> {
    let scanline = register_u16(registers, "scanline")?;
    let dot = register_u16(registers, "dot")?;
    Some(PpuPosition { scanline, dot })
}

fn register_u16(registers: &RegisterMap, key: &str) -> Option<u16> {
    match registers.get(key)?.value {
        RegisterValue::U16(value) => Some(value),
        RegisterValue::U8(value) => Some(value as u16),
        RegisterValue::U32(value) => Some(value as u16),
        RegisterValue::U64(value) => Some(value as u16),
        _ => None,
    }
}
