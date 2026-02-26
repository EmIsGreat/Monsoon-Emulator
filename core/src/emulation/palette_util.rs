use serde::{Deserialize, Serialize};

use crate::util::{Hashable, ToBytes, compute_hash};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Default)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
        }
    }

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

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct RgbPalette {
    pub colors: [[RgbColor; 64]; 8],
}

impl Default for RgbPalette {
    fn default() -> Self { parse_palette_from_bytes(DEFAULT_PALETTE) }
}

static DEFAULT_PALETTE: &[u8] = include_bytes!("../../../core/assets/2C02G_wiki.pal");

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
