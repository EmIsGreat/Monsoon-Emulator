use std::fmt::{Debug, Formatter};

use monsoon_core::emulation::palette_util::{RgbColor, RgbPalette};
use monsoon_core::emulation::ppu_util::{TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH};
use monsoon_core::emulation::screen_renderer::ScreenRenderer;

pub const WGPU_RENDERER_ID: &str = "WgpuPaletteShader";

/// `ScreenRenderer` adapter used to integrate the wgpu callback renderer with
/// the existing runtime renderer selection workflow.
///
/// This renderer does not produce CPU-side RGB output. When selected, the
/// frontend routes the main output pane through the dedicated wgpu callback
/// path.
#[derive(Default)]
pub struct WgpuScreenRenderer;

impl WgpuScreenRenderer {
    pub fn new() -> Self { Self }
}

impl Debug for WgpuScreenRenderer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get_display_name())
    }
}

impl ScreenRenderer for WgpuScreenRenderer {
    fn process_frame(&mut self, _: &[u16]) -> Option<&[RgbColor]> { None }

    fn set_palette(&mut self, _: RgbPalette) {}

    fn get_width(&self) -> usize { TOTAL_OUTPUT_WIDTH }

    fn get_height(&self) -> usize { TOTAL_OUTPUT_HEIGHT }

    fn get_id(&self) -> &'static str { WGPU_RENDERER_ID }

    fn get_display_name(&self) -> &'static str { "WGPU Palette Shader Renderer" }
}
