//! PPU-related types and constants used by frontends and renderers.
//!
//! This module provides the public data types and dimension constants from the
//! PPU (Picture Processing Unit) that downstream crates need for rendering,
//! debug views, and display layout. The actual PPU emulation internals remain
//! in the crate-private `ppu` module.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

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
/// PPU address at which palette RAM ends (`$3FFF`).
pub const PALETTE_RAM_END_ADDRESS: u16 = 0x3FFF;
/// Size of a single tile in pixels (8×8).
pub const TILE_SIZE: usize = 8;

/// Number of nametables in the PPU address space (4).
pub const NAMETABLE_COUNT: usize = 4;
/// Number of tile rows per nametable (30).
pub const NAMETABLE_ROWS: usize = 30;
/// Number of tile columns per nametable (32).
pub const NAMETABLE_COLS: usize = 32;
/// Max number of sprites possible
pub const SPRITE_COUNT: usize = 64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RegisterValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    Bool(bool),
    Text(String),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RegisterFormat {
    Hex,
    Binary,
    Decimal,
    Bool,
    Text,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RegisterEntry {
    pub value: RegisterValue,
    pub format: RegisterFormat,
}

impl RegisterEntry {
    pub fn new(value: RegisterValue, format: RegisterFormat) -> Self {
        Self {
            value,
            format,
        }
    }

    pub fn formatted_value(&self) -> String {
        match (&self.value, self.format) {
            (RegisterValue::U8(value), RegisterFormat::Hex) => format!("0x{value:02X}"),
            (RegisterValue::U16(value), RegisterFormat::Hex) => format!("0x{value:04X}"),
            (RegisterValue::U32(value), RegisterFormat::Hex) => format!("0x{value:08X}"),
            (RegisterValue::U64(value), RegisterFormat::Hex) => format!("0x{value:016X}"),
            (RegisterValue::U8(value), RegisterFormat::Binary) => format!("0b{value:08b}"),
            (RegisterValue::U16(value), RegisterFormat::Binary) => format!("0b{value:016b}"),
            (RegisterValue::U32(value), RegisterFormat::Binary) => format!("0b{value:032b}"),
            (RegisterValue::U64(value), RegisterFormat::Binary) => format!("0b{value:064b}"),
            (RegisterValue::U8(value), RegisterFormat::Decimal) => value.to_string(),
            (RegisterValue::U16(value), RegisterFormat::Decimal) => value.to_string(),
            (RegisterValue::U32(value), RegisterFormat::Decimal) => value.to_string(),
            (RegisterValue::U64(value), RegisterFormat::Decimal) => value.to_string(),
            (RegisterValue::Bool(value), RegisterFormat::Bool) => value.to_string(),
            (RegisterValue::Text(value), RegisterFormat::Text) => value.clone(),
            (RegisterValue::Bool(value), RegisterFormat::Decimal) => (*value as u8).to_string(),
            (RegisterValue::Bool(value), RegisterFormat::Hex) => format!("0x{:02X}", *value as u8),
            (RegisterValue::Bool(value), RegisterFormat::Binary) => {
                format!("0b{:08b}", *value as u8)
            }
            (RegisterValue::Text(value), _) => value.clone(),
            (value, _) => match value {
                RegisterValue::U8(value) => value.to_string(),
                RegisterValue::U16(value) => value.to_string(),
                RegisterValue::U32(value) => value.to_string(),
                RegisterValue::U64(value) => value.to_string(),
                RegisterValue::Bool(value) => value.to_string(),
                RegisterValue::Text(value) => value.clone(),
            },
        }
    }
}

pub type RegisterMap = HashMap<String, RegisterEntry>;
pub type MapperRegisterTables = HashMap<String, RegisterMap>;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RegisterDebugData {
    pub ppu: RegisterMap,
    pub apu: RegisterMap,
    pub mapper: MapperRegisterTables,
}

/// Describes a category of debug data that can be fetched from the emulator.
///
/// Used by frontends to request and receive PPU debug information such as
/// palette colors, pattern table tiles, or nametable layouts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EmulatorFetchable {
    /// Palette color data (4 bytes × 8 palettes).
    Palettes(Option<Box<PaletteData>>),
    /// Pattern table tile data for all 512 tiles.
    Tiles(Option<Box<[TileData; TILE_COUNT]>>),
    /// Nametable layout data for all 4 nametables.
    Nametables(Option<Box<NametableData>>),
    Sprites(Option<Box<SpriteData>>),
    SoamSprites(Option<Box<SoamData>>),
    Registers(Option<Box<RegisterDebugData>>),
}

impl Hash for EmulatorFetchable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let discriminant = match self {
            EmulatorFetchable::Palettes(_) => 0u8,
            EmulatorFetchable::Tiles(_) => 1,
            EmulatorFetchable::Nametables(_) => 2,
            EmulatorFetchable::Sprites(_) => 3,
            EmulatorFetchable::SoamSprites(_) => 4,
            EmulatorFetchable::Registers(_) => 5,
        };
        discriminant.hash(state);
    }
}

impl EmulatorFetchable {
    /// Returns an empty variant of the same kind (with `None` payload).
    #[inline]
    pub fn get_empty(emulator_fetchable: &EmulatorFetchable) -> EmulatorFetchable {
        match emulator_fetchable {
            EmulatorFetchable::Palettes(_) => EmulatorFetchable::Palettes(None),
            EmulatorFetchable::Tiles(_) => EmulatorFetchable::Tiles(None),
            EmulatorFetchable::Nametables(_) => EmulatorFetchable::Nametables(None),
            EmulatorFetchable::Sprites(_) => EmulatorFetchable::Sprites(None),
            EmulatorFetchable::SoamSprites(_) => EmulatorFetchable::SoamSprites(None),
            EmulatorFetchable::Registers(_) => EmulatorFetchable::Registers(None),
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
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
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

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SpriteData {
    pub sprites: [Sprite; SPRITE_COUNT],
    pub mode: SpriteMode,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SoamData {
    pub sprites: [Sprite; 8],
    pub mode: SpriteMode,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Sprite {
    pub y_pos: u16,
    pub x_pos: u16,
    pub tile: u16,
    pub bottom_tile: u16,
    pub palette: u8,
    pub priority: bool,
    pub h_flip: bool,
    pub v_flip: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum SpriteMode {
    #[default]
    SMALL,
    TALL,
}

impl SpriteMode {
    pub fn get_height_mult(&self) -> u8 {
        match self {
            SpriteMode::SMALL => 1,
            SpriteMode::TALL => 2,
        }
    }
}
