use std::fmt::{Debug, Formatter};

use monsoon_core::emulation::palette_util::{RgbColor, RgbPalette};
use monsoon_core::emulation::ppu::{TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH};
use monsoon_core::emulation::screen_renderer::ScreenRenderer;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// Number of colors in the NES palette (64 base colors)
const PALETTE_COLORS: usize = 64;
/// Number of emphasis combinations (8 = 2^3 for R, G, B emphasis bits)
const EMPHASIS_COMBINATIONS: usize = 8;
/// Total size of the flat palette lookup table
const FLAT_PALETTE_SIZE: usize = PALETTE_COLORS * EMPHASIS_COMBINATIONS;
/// Bitmask for extracting the 9-bit palette index (6 color bits + 3 emphasis bits)
const PALETTE_INDEX_MASK: usize = FLAT_PALETTE_SIZE - 1;

/// A flat palette structure optimized for lookup-table based rendering.
///
/// The NES PPU outputs a 9-bit value for each pixel:
/// - Bits 0-5 (6 bits): Color index (0-63 from the NES palette)
/// - Bits 6-8 (3 bits): Emphasis bits (R, G, B emphasis from PPU mask register)
///
/// This flat structure allows O(1) lookup using the raw index value.
#[derive(Clone, Serialize, Deserialize)]
struct FlatPalette {
    #[serde(with = "BigArray")]
    palette: [RgbColor; FLAT_PALETTE_SIZE],
}

impl From<RgbPalette> for FlatPalette {
    fn from(palette: RgbPalette) -> Self {
        let mut flat = [RgbColor::default(); FLAT_PALETTE_SIZE];

        for color in 0..PALETTE_COLORS {
            for emph in 0..EMPHASIS_COMBINATIONS {
                flat[color | (emph << 6)] = palette.colors[emph][color];
            }
        }

        Self {
            palette: flat,
        }
    }
}

/// Lookup table–based palette renderer.
///
/// This renderer pre-computes a flat 512-entry lookup table mapping each
/// possible 9-bit NES palette index (6 color bits + 3 emphasis bits) to
/// an [`RgbColor`]. Rendering is a simple O(1) array lookup per pixel,
/// making this the fastest available renderer.
///
/// # Example
///
/// ```rust,no_run
/// use monsoon_default_renderers::LookupPaletteRenderer;
/// use monsoon_core::emulation::screen_renderer::ScreenRenderer;
/// use monsoon_core::emulation::palette_util::RgbPalette;
///
/// let mut renderer = LookupPaletteRenderer::default();
///
/// // Optionally load a custom palette
/// let palette = RgbPalette::default();
/// renderer.set_palette(palette);
///
/// // Convert a pixel buffer to RGB
/// # let pixel_buffer: &[u16] = &[];
/// let rgb = renderer.buffer_to_image(pixel_buffer);
/// ```
#[derive(Clone, Serialize, Deserialize)]
pub struct LookupPaletteRenderer {
    palette: FlatPalette,
    image: Vec<RgbColor>,
}

impl Default for LookupPaletteRenderer {
    fn default() -> Self { Self::new() }
}

impl LookupPaletteRenderer {
    /// Create a new renderer with the default palette
    pub fn new() -> Self {
        Self {
            palette: RgbPalette::default().into(),
            image: vec![],
        }
    }
}

impl Debug for LookupPaletteRenderer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get_display_name())
    }
}

impl ScreenRenderer for LookupPaletteRenderer {
    fn buffer_to_image(&mut self, buffer: &[u16]) -> &[RgbColor] {
        if self.image.len() != buffer.len() {
            self.image = Vec::with_capacity(buffer.len());
        };

        self.image.clear();
        for x in buffer.iter() {
            self.image
                .push(self.palette.palette[(*x as usize) & PALETTE_INDEX_MASK])
        }

        &self.image
    }

    fn set_palette(&mut self, rgb_palette: RgbPalette) { self.palette = rgb_palette.into(); }

    fn get_width(&self) -> usize { TOTAL_OUTPUT_WIDTH }

    fn get_height(&self) -> usize { TOTAL_OUTPUT_HEIGHT }

    fn get_id(&self) -> &'static str { "PaletteLookup" }

    fn get_display_name(&self) -> &'static str { "Palette Lookup Renderer" }
}
