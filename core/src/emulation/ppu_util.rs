//! PPU-related types and constants used by frontends and renderers.
//!
//! This module provides the public data types and dimension constants from the
//! PPU (Picture Processing Unit) that downstream crates need for rendering,
//! debug views, and display layout. The actual PPU emulation internals remain
//! in the crate-private `ppu` module.

/// Total output width of the NES frame in pixels (256).
pub const TOTAL_OUTPUT_WIDTH: usize = 256;
/// Total output height of the NES frame in pixels (240).
pub const TOTAL_OUTPUT_HEIGHT: usize = 240;

/// Total number of tiles across both pattern tables (512).
pub const TILE_COUNT: usize = 512;
/// Number of palettes (8: 4 background + 4 sprite).
pub const PALETTE_COUNT: usize = 8;
/// PPU address at which palette RAM begins (`$3F00`).
pub const PALETTE_RAM_START_ADDRESS: u16 = 0x3F00;
/// Size of a single tile in pixels (8×8).
pub const TILE_SIZE: usize = 8;

/// Number of nametables in the PPU address space (4).
pub const NAMETABLE_COUNT: usize = 4;
/// Number of tile rows per nametable (30).
pub const NAMETABLE_ROWS: usize = 30;
/// Number of tile columns per nametable (32).
pub const NAMETABLE_COLS: usize = 32;

/// Describes a category of debug data that can be fetched from the emulator.
///
/// Used by frontends to request and receive PPU debug information such as
/// palette colors, pattern table tiles, or nametable layouts.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum EmulatorFetchable {
    /// Palette color data (4 bytes × 8 palettes).
    Palettes(Option<Box<PaletteData>>),
    /// Pattern table tile data for all 512 tiles.
    Tiles(Option<Box<[TileData; TILE_COUNT]>>),
    /// Nametable layout data for all 4 nametables.
    Nametables(Option<Box<NametableData>>),
}

impl EmulatorFetchable {
    /// Returns an empty variant of the same kind (with `None` payload).
    #[inline]
    pub fn get_empty(emulator_fetchable: &EmulatorFetchable) -> EmulatorFetchable {
        match emulator_fetchable {
            EmulatorFetchable::Palettes(_) => EmulatorFetchable::Palettes(None),
            EmulatorFetchable::Tiles(_) => EmulatorFetchable::Tiles(None),
            EmulatorFetchable::Nametables(_) => EmulatorFetchable::Nametables(None),
        }
    }

    /// Returns true if this fetchable should only be fetched when the emulator
    /// notifies that the data has changed (passive), rather than on a regular
    /// interval (active).
    ///
    /// Passive fetches reduce CPU overhead for data that rarely changes.
    #[inline]
    pub fn is_passive(&self) -> bool {
        matches!(
            self,
            EmulatorFetchable::Palettes(_) | EmulatorFetchable::Tiles(_)
        )
    }
}

/// Snapshot of all 8 NES palettes (4 background + 4 sprite).
///
/// Each palette contains 4 color indices into the system palette.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PaletteData {
    /// 8 palettes of 4 color indices each.
    pub colors: [[u8; 4]; 8],
}

/// Snapshot of all 4 nametable layouts.
///
/// Contains tile indices and palette attribute data for the full 2×2
/// nametable arrangement.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct NametableData {
    /// Tile indices for each nametable (30 rows × 32 columns each).
    pub tiles: [[u16; NAMETABLE_ROWS * NAMETABLE_COLS]; NAMETABLE_COUNT],
    /// Palette attribute bytes for each nametable.
    pub palettes: [[u8; 64]; NAMETABLE_COUNT],
}

/// Raw tile data from a pattern table entry.
///
/// Each tile is 8×8 pixels stored as two bit planes. Combine `plane_0`
/// and `plane_1` to get 2-bit color indices per pixel.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct TileData {
    /// CHR ROM/RAM address of this tile.
    pub address: u16,
    /// Low bit plane (8 rows × 8 bits packed into a `u64`).
    pub plane_0: u64,
    /// High bit plane (8 rows × 8 bits packed into a `u64`).
    pub plane_1: u64,
}
