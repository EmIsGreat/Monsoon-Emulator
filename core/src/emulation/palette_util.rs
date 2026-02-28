//! NES color palette types and parsing.
//!
//! The NES PPU uses a fixed palette of 64 base colors, with 8 emphasis
//! combinations for a total of 512 possible output colors. This module
//! provides types for representing these colors ([`RgbColor`], [`RgbPalette`])
//! and for loading palette data from `.pal` files ([`parse_palette_from_bytes`]).
//!
//! A default palette based on the 2C02G PPU is embedded in the library.

use serde::{Deserialize, Serialize};

use crate::util::{Hashable, ToBytes, compute_hash};

/// An RGB color with 8 bits per channel.
///
/// This is the output type produced by [`ScreenRenderer`](crate::emulation::screen_renderer::ScreenRenderer)
/// implementations when converting NES palette indices to displayable colors.
///
/// # Example
///
/// ```rust
/// use monsoon_core::emulation::palette_util::RgbColor;
///
/// let red = RgbColor::new(255, 0, 0);
/// let (r, g, b) = red.to_tuple();
/// assert_eq!((r, g, b), (255, 0, 0));
///
/// let blue: RgbColor = (0, 0, 255).into();
/// assert_eq!(blue.b, 255);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Default)]
pub struct RgbColor {
    /// Red channel (0-255).
    pub r: u8,
    /// Green channel (0-255).
    pub g: u8,
    /// Blue channel (0-255).
    pub b: u8,
}

impl RgbColor {
    /// Creates a new [`RgbColor`] from individual channel values.
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
        }
    }

    /// Converts this color to a `(r, g, b)` tuple.
    pub fn to_tuple(self) -> (u8, u8, u8) { (self.r, self.g, self.b) }
}

impl From<(u8, u8, u8)> for RgbColor {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self {
            r,
            g,
            b,
        }
    }
}

/// A complete NES RGB palette with emphasis support.
///
/// The palette contains 64 base colors × 8 emphasis combinations = 512 total
/// entries. Index as `colors[emphasis][color_index]` where:
///
/// - `emphasis` is a 3-bit value (0-7) from the PPU mask register bits 5-7.
/// - `color_index` is a 6-bit value (0-63) from the PPU output.
///
/// Use [`RgbPalette::default()`] to get the built-in 2C02G palette, or
/// [`parse_palette_from_bytes()`] to load a custom `.pal` file.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct RgbPalette {
    /// The color table: `colors[emphasis_bits][color_index]`.
    pub colors: [[RgbColor; 64]; 8],
}

impl Default for RgbPalette {
    fn default() -> Self { parse_palette_from_bytes(DEFAULT_PALETTE) }
}

static DEFAULT_PALETTE: &[u8] = include_bytes!("../../../core/assets/2C02G_wiki.pal");

/// Parses a `.pal` palette file from raw bytes into an [`RgbPalette`].
///
/// The expected format is a flat array of RGB triplets: 64 colors × 8
/// emphasis variants × 3 bytes (R, G, B) = 1,536 bytes total. If the
/// provided data is shorter than expected, missing entries are filled
/// from the built-in default palette.
///
/// # Example
///
/// ```rust
/// use monsoon_core::emulation::palette_util::{RgbPalette, parse_palette_from_bytes};
///
/// // Load the default palette explicitly
/// let palette = RgbPalette::default();
/// assert_eq!(palette.colors[0].len(), 64);
/// ```
pub fn parse_palette_from_bytes(bytes: &[u8]) -> RgbPalette {
    let mut colors: [[RgbColor; 64]; 8] = [[RgbColor::default(); 64]; 8];

    for (palette_index, palette) in colors.iter_mut().enumerate() {
        for (color_index, color) in palette.iter_mut().enumerate() {
            let base_index = ((palette_index * 64) + color_index) * 3;
            let read_color: RgbColor = if let (Some(r), Some(g), Some(b)) = (
                bytes.get(base_index),
                bytes.get(base_index + 1),
                bytes.get(base_index + 2),
            ) {
                RgbColor {
                    r: *r,
                    g: *g,
                    b: *b,
                }
            } else {
                RgbColor {
                    r: DEFAULT_PALETTE[base_index],
                    g: DEFAULT_PALETTE[base_index + 1],
                    b: DEFAULT_PALETTE[base_index + 2],
                }
            };

            *color = read_color;
        }
    }

    RgbPalette {
        colors,
    }
}

impl ToBytes for RgbPalette {
    fn to_bytes(&self, _: Option<String>) -> Vec<u8> {
        self.colors
            .iter()
            .flatten()
            .flat_map(|c| [c.r, c.g, c.b])
            .collect()
    }
}

impl Hashable for RgbPalette {
    /// Compute a fast hash of the given data for change detection.
    /// Uses FNV-1a algorithm which is fast and has good distribution.
    #[inline]
    fn hash(&self) -> u64 {
        let bytes = self.to_bytes(None);
        compute_hash(&bytes[..])
    }
}
