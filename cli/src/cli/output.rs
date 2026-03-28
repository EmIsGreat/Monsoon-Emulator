//! Output formatting system for CLI memory dumps.
//!
//! This module provides an extensible output format system using traits.
//! Adding a new output format requires:
//! 1. Add a variant to `OutputFormat` enum
//! 2. Implement the `MemoryFormatter` trait for the new format
//! 3. Register it in `OutputFormat::formatter()`
//!
//! # Interpreted Data
//!
//! OAM and nametable dumps include both raw data and interpreted structures:
//!
//! - **OAM**: 64 sprites with position, tile index, attributes, flip flags
//! - **Nametables**: 4 nametables with tile IDs, attribute tables, and per-tile
//!   palettes
//!
//! # Example: Adding a new format
//!
//! ```rust,ignore
//! // 1. Add enum variant in args.rs
//! pub enum OutputFormat {
//!     Hex,
//!     Json,
//!     Toml,
//!     Binary,
//!     Xml,  // New format
//! }
//!
//! // 2. Implement the formatter
//! pub struct XmlFormatter;
//!
//! impl MemoryFormatter for XmlFormatter {
//!     fn format(&self, dump: &MemoryDump) -> Result<Vec<u8>, String> {
//!         // ... format as XML ...
//!     }
//!
//!     fn file_extension(&self) -> &'static str {
//!         "xml"
//!     }
//! }
//!
//! // 3. Register in OutputFormat::formatter()
//! OutputFormat::Xml => Box::new(XmlFormatter),
//! ```

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

use crate::cli::args::OutputFormat;

// =============================================================================
// OAM Interpretation Structures
// =============================================================================

/// A single sprite entry from OAM (4 bytes interpreted).
///
/// NES OAM format per sprite:
/// - Byte 0: Y position (actual Y = value + 1, values 0xEF-0xFF hide sprite)
/// - Byte 1: Tile index
/// - Byte 2: Attributes (palette, priority, flip)
/// - Byte 3: X position
#[derive(Debug, Clone, Serialize)]
pub struct OamSprite {
    /// Sprite index (0-63)
    pub index: u8,
    /// X position (0-255, values 0xF9-0xFF partially offscreen right)
    pub x: u8,
    /// Y position (raw value; actual screen Y = y + 1)
    pub y: u8,
    /// Tile index number
    pub tile: u8,
    /// Palette index (0-3, actual palette 4-7)
    pub palette: u8,
    /// Priority: false = in front of background, true = behind background
    pub behind_background: bool,
    /// Flip sprite horizontally
    pub flip_h: bool,
    /// Flip sprite vertically
    pub flip_v: bool,
    /// Whether sprite is visible (Y < 0xEF)
    pub visible: bool,
    /// Raw 4 bytes for this sprite
    pub raw: [u8; 4],
}

impl OamSprite {
    /// Create a sprite from 4 raw OAM bytes.
    ///
    /// # Panics
    /// Panics if bytes slice has fewer than 4 elements.
    pub fn from_bytes(index: u8, bytes: &[u8]) -> Self {
        assert!(
            bytes.len() >= 4,
            "OAM sprite requires at least 4 bytes, got {}",
            bytes.len()
        );
        let y = bytes[0];
        let tile = bytes[1];
        let attr = bytes[2];
        let x = bytes[3];

        Self {
            index,
            x,
            y,
            tile,
            palette: attr & 0x03,
            behind_background: (attr & 0x20) != 0,
            flip_h: (attr & 0x40) != 0,
            flip_v: (attr & 0x80) != 0,
            visible: y < 0xEF,
            raw: [bytes[0], bytes[1], bytes[2], bytes[3]],
        }
    }
}

/// Interpreted OAM data containing all 64 sprites.
#[derive(Debug, Clone, Serialize)]
pub struct InterpretedOam {
    /// Total number of sprites (always 64)
    pub sprite_count: u8,
    /// Number of visible sprites (Y < 0xEF)
    pub visible_count: u8,
    /// All 64 sprites with interpreted fields
    pub sprites: Vec<OamSprite>,
}

impl InterpretedOam {
    /// Create interpreted OAM from raw OAM data (up to 256 bytes for 64
    /// sprites).
    pub fn from_raw(data: &[u8]) -> Self {
        let mut sprites = Vec::with_capacity(64);
        let mut visible_count = 0u8;

        for i in 0..64 {
            let offset = i * 4;
            if offset + 4 <= data.len() {
                let sprite = OamSprite::from_bytes(i as u8, &data[offset..offset + 4]);
                if sprite.visible {
                    visible_count += 1;
                }
                sprites.push(sprite);
            }
        }

        Self {
            sprite_count: sprites.len() as u8,
            visible_count,
            sprites,
        }
    }
}

// =============================================================================
// Nametable Interpretation Structures
// =============================================================================

/// A single nametable (32x30 tiles + 64-byte attribute table).
#[derive(Debug, Clone, Serialize)]
pub struct InterpretedNametable {
    /// Nametable index (0-3)
    pub index: u8,
    /// Base address in PPU memory ($2000, $2400, $2800, $2C00)
    pub base_address: String,
    /// 32x30 = 960 tile indices
    pub tiles: Vec<Vec<u8>>,
    /// 8x8 = 64 attribute bytes (each controls 4x4 tile area)
    pub attributes: Vec<u8>,
    /// Per-tile palette indices (derived from attribute table)
    pub tile_palettes: Vec<Vec<u8>>,
}

impl InterpretedNametable {
    /// Create interpreted nametable from raw 1024-byte nametable data.
    ///
    /// Layout:
    /// - Bytes 0x000-0x3BF: 960 tile indices (32 columns x 30 rows)
    /// - Bytes 0x3C0-0x3FF: 64 attribute bytes
    pub fn from_raw(index: u8, data: &[u8]) -> Self {
        let base_addresses = [0x2000u16, 0x2400, 0x2800, 0x2C00];
        let base = base_addresses[index as usize % 4];

        // Extract tiles as 30 rows of 32 columns
        let mut tiles = Vec::with_capacity(30);
        for row in 0..30 {
            let start = row * 32;
            let end = start + 32;
            if end <= data.len() {
                tiles.push(data[start..end].to_vec());
            } else {
                tiles.push(vec![0; 32]);
            }
        }

        // Extract attribute table (64 bytes starting at offset 0x3C0)
        let attr_start = 0x3C0;
        let attributes = if attr_start + 64 <= data.len() {
            data[attr_start..attr_start + 64].to_vec()
        } else {
            vec![0; 64]
        };

        // Calculate per-tile palette indices from attribute table
        // Each attribute byte controls a 4x4 tile area (32x32 pixels)
        // Bits: 76543210
        //       ||||||++- Top-left 2x2 tiles
        //       ||||++--- Top-right 2x2 tiles
        //       ||++----- Bottom-left 2x2 tiles
        //       ++------- Bottom-right 2x2 tiles
        let mut tile_palettes = Vec::with_capacity(30);
        for row in 0..30 {
            let mut row_palettes = Vec::with_capacity(32);
            for col in 0..32 {
                // Which attribute byte controls this tile
                let attr_col = col / 4;
                let attr_row = row / 4;
                let attr_index = attr_row * 8 + attr_col;

                if attr_index < attributes.len() {
                    let attr = attributes[attr_index];
                    // Which 2x2 quadrant within the 4x4 area
                    let quadrant_x = (col % 4) / 2;
                    let quadrant_y = (row % 4) / 2;
                    let shift = (quadrant_y * 2 + quadrant_x) * 2;
                    let palette = (attr >> shift) & 0x03;
                    row_palettes.push(palette);
                } else {
                    row_palettes.push(0);
                }
            }
            tile_palettes.push(row_palettes);
        }

        Self {
            index,
            base_address: format!("0x{:04X}", base),
            tiles,
            attributes,
            tile_palettes,
        }
    }
}

/// Interpreted nametable data containing all 4 nametables.
#[derive(Debug, Clone, Serialize)]
pub struct InterpretedNametables {
    /// Total size in bytes (4096 for 4 nametables)
    pub total_size: usize,
    /// All 4 nametables
    pub nametables: Vec<InterpretedNametable>,
}

impl InterpretedNametables {
    /// Create interpreted nametables from raw data.
    ///
    /// Expects data for nametables at $2000-$2FFF (4 x 1024 bytes).
    pub fn from_raw(data: &[u8]) -> Self {
        let mut nametables = Vec::with_capacity(4);

        for i in 0..4 {
            let start = i * 0x400;
            let end = start + 0x400;
            if end <= data.len() {
                nametables.push(InterpretedNametable::from_raw(i as u8, &data[start..end]));
            } else if start < data.len() {
                // Partial nametable
                let partial = &data[start..];
                let mut padded = partial.to_vec();
                padded.resize(0x400, 0);
                nametables.push(InterpretedNametable::from_raw(i as u8, &padded));
            }
        }

        Self {
            total_size: data.len(),
            nametables,
        }
    }
}

// =============================================================================
// Memory Dump Data Structure
// =============================================================================

/// A memory dump with metadata and optional interpreted data.
///
/// This is the common data structure passed to all formatters.
/// For OAM and nametable dumps, interpreted data is also included.
#[derive(Debug, Clone)]
pub struct MemoryDump {
    /// Type of memory (cpu, ppu, oam, nametables)
    pub mem_type: MemoryType,
    /// Start address
    pub start_addr: u16,
    /// End address
    pub end_addr: u16,
    /// Raw memory bytes
    pub data: Vec<u8>,
    /// Interpreted OAM data (only for OAM dumps)
    pub interpreted_oam: Option<InterpretedOam>,
    /// Interpreted nametable data (only for nametable dumps)
    pub interpreted_nametables: Option<InterpretedNametables>,
}

/// Type of memory being dumped
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    /// CPU address space
    Cpu,
    /// PPU address space
    Ppu,
    /// OAM (sprite) memory
    Oam,
    /// Nametables
    Nametables,
    /// Palette RAM
    PaletteRam,
}

impl MemoryType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::Cpu => "cpu",
            MemoryType::Ppu => "ppu",
            MemoryType::Oam => "oam",
            MemoryType::Nametables => "nametables",
            MemoryType::PaletteRam => "palette_ram",
        }
    }
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl MemoryDump {
    /// Create a new memory dump
    pub fn new(mem_type: MemoryType, start_addr: u16, data: Vec<u8>) -> Self {
        let end_addr = if data.is_empty() {
            start_addr
        } else {
            start_addr.saturating_add((data.len() - 1) as u16)
        };
        Self {
            mem_type,
            start_addr,
            end_addr,
            data,
            interpreted_oam: None,
            interpreted_nametables: None,
        }
    }

    /// Create CPU memory dump
    pub fn cpu(start_addr: u16, data: Vec<u8>) -> Self {
        Self::new(MemoryType::Cpu, start_addr, data)
    }

    /// Create PPU memory dump
    pub fn ppu(start_addr: u16, data: Vec<u8>) -> Self {
        Self::new(MemoryType::Ppu, start_addr, data)
    }

    /// Create OAM memory dump with interpreted sprite data.
    pub fn oam(data: Vec<u8>) -> Self {
        let interpreted = InterpretedOam::from_raw(&data);
        let mut dump = Self::new(MemoryType::Oam, 0, data);
        dump.interpreted_oam = Some(interpreted);
        dump
    }

    /// Create nametables memory dump with interpreted data.
    pub fn nametables(data: Vec<u8>) -> Self {
        let interpreted = InterpretedNametables::from_raw(&data);
        let mut dump = Self::new(MemoryType::Nametables, 0x2000, data);
        dump.interpreted_nametables = Some(interpreted);
        dump
    }

    /// Create palette RAM memory dump.
    /// Palette RAM is 32 bytes at PPU addresses $3F00-$3F1F.
    pub fn palette_ram(data: Vec<u8>) -> Self { Self::new(MemoryType::PaletteRam, 0x3F00, data) }
}

// =============================================================================
// Formatter Trait
// =============================================================================

/// Trait for memory dump formatters.
///
/// Implement this trait to add a new output format.
pub trait MemoryFormatter: Send + Sync {
    /// Format a memory dump into bytes ready to be written.
    fn format(&self, dump: &MemoryDump) -> Result<Vec<u8>, String>;

    /// Get the file extension for this format (without leading dot).
    fn file_extension(&self) -> &'static str;

    /// Whether this format is human-readable (affects stdout display).
    fn is_text(&self) -> bool { true }
}

// =============================================================================
// Built-in Formatters
// =============================================================================

/// Hexadecimal dump formatter (traditional hex dump format)
///
/// For OAM dumps, includes both raw hex and human-readable sprite
/// interpretation. For nametable dumps, includes both raw hex and nametable
/// summary.
pub struct HexFormatter;

impl MemoryFormatter for HexFormatter {
    fn format(&self, dump: &MemoryDump) -> Result<Vec<u8>, String> {
        let mut output = String::new();

        // For OAM, add interpreted header
        if let Some(ref oam) = dump.interpreted_oam {
            output.push_str("=== OAM Interpretation ===\n");
            output.push_str(&format!(
                "Total sprites: {}, Visible: {}\n\n",
                oam.sprite_count, oam.visible_count
            ));
            output.push_str("Idx |  X  |  Y  | Tile | Pal | Pri | FlipH | FlipV | Visible\n");
            output.push_str("----+-----+-----+------+-----+-----+-------+-------+--------\n");
            for sprite in &oam.sprites {
                output.push_str(&format!(
                    "{:3} | {:3} | {:3} | 0x{:02X} |  {}  | {} |   {}   |   {}   | {}\n",
                    sprite.index,
                    sprite.x,
                    sprite.y,
                    sprite.tile,
                    sprite.palette,
                    if sprite.behind_background { "B" } else { "F" },
                    if sprite.flip_h { "Y" } else { "N" },
                    if sprite.flip_v { "Y" } else { "N" },
                    if sprite.visible { "Yes" } else { "No" }
                ));
            }
            output.push_str("\n=== Raw OAM Data ===\n");
        }

        // For nametables, add interpreted header
        if let Some(ref nt) = dump.interpreted_nametables {
            output.push_str("=== Nametables Interpretation ===\n");
            output.push_str(&format!("Total size: {} bytes\n\n", nt.total_size));
            for nametable in &nt.nametables {
                output.push_str(&format!(
                    "Nametable {} (base: {})\n",
                    nametable.index, nametable.base_address
                ));
                output.push_str("  Tiles (32x30 grid, showing first 8 rows):\n");
                for (row_idx, row) in nametable.tiles.iter().take(8).enumerate() {
                    output.push_str(&format!("  Row {:2}: ", row_idx));
                    for tile in row.iter().take(32) {
                        output.push_str(&format!("{:02X} ", tile));
                    }
                    output.push('\n');
                }
                if nametable.tiles.len() > 8 {
                    output.push_str("  ... (22 more rows)\n");
                }
                output.push('\n');
            }
            output.push_str("=== Raw Nametable Data ===\n");
        }

        // Raw hex dump
        for (i, chunk) in dump.data.chunks(16).enumerate() {
            let line = format!(
                "{:04X}: {}\n",
                dump.start_addr as usize + i * 16,
                chunk
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            output.push_str(&line);
        }
        Ok(output.into_bytes())
    }

    fn file_extension(&self) -> &'static str { "hex" }
}

/// Raw binary formatter
pub struct BinaryFormatter;

impl MemoryFormatter for BinaryFormatter {
    fn format(&self, dump: &MemoryDump) -> Result<Vec<u8>, String> { Ok(dump.data.clone()) }

    fn file_extension(&self) -> &'static str { "bin" }

    fn is_text(&self) -> bool { false }
}

/// JSON formatter
pub struct JsonFormatter;

/// Structure for JSON/TOML serialization of basic memory dumps
#[derive(Serialize)]
struct MemoryDumpOutput {
    memory_dump: MemoryDumpData,
}

#[derive(Serialize)]
struct MemoryDumpData {
    #[serde(rename = "type")]
    mem_type: String,
    start: String,
    end: String,
    data: Vec<String>,
}

/// Structure for JSON/TOML serialization of OAM dumps with interpretation
#[derive(Serialize)]
struct OamDumpOutput {
    oam_dump: OamDumpData,
}

#[derive(Serialize)]
struct OamDumpData {
    #[serde(rename = "type")]
    mem_type: String,
    size: usize,
    raw_data: Vec<String>,
    interpretation: InterpretedOam,
}

/// Structure for JSON/TOML serialization of nametable dumps with interpretation
#[derive(Serialize)]
struct NametablesDumpOutput {
    nametables_dump: NametablesDumpData,
}

#[derive(Serialize)]
struct NametablesDumpData {
    #[serde(rename = "type")]
    mem_type: String,
    start: String,
    end: String,
    raw_data: Vec<String>,
    interpretation: InterpretedNametables,
}

impl MemoryFormatter for JsonFormatter {
    fn format(&self, dump: &MemoryDump) -> Result<Vec<u8>, String> {
        let data_hex: Vec<String> = dump.data.iter().map(|b| format!("0x{:02X}", b)).collect();

        let json_str = match dump.mem_type {
            MemoryType::Oam => {
                if let Some(ref interp) = dump.interpreted_oam {
                    let output = OamDumpOutput {
                        oam_dump: OamDumpData {
                            mem_type: dump.mem_type.to_string(),
                            size: dump.data.len(),
                            raw_data: data_hex,
                            interpretation: interp.clone(),
                        },
                    };
                    serde_json::to_string_pretty(&output)
                        .map_err(|e| format!("Failed to serialize JSON: {}", e))?
                } else {
                    // Fallback to basic output
                    let output = MemoryDumpOutput {
                        memory_dump: MemoryDumpData {
                            mem_type: dump.mem_type.to_string(),
                            start: format!("0x{:04X}", dump.start_addr),
                            end: format!("0x{:04X}", dump.end_addr),
                            data: data_hex,
                        },
                    };
                    serde_json::to_string_pretty(&output)
                        .map_err(|e| format!("Failed to serialize JSON: {}", e))?
                }
            }
            MemoryType::Nametables => {
                if let Some(ref interp) = dump.interpreted_nametables {
                    let output = NametablesDumpOutput {
                        nametables_dump: NametablesDumpData {
                            mem_type: dump.mem_type.to_string(),
                            start: format!("0x{:04X}", dump.start_addr),
                            end: format!("0x{:04X}", dump.end_addr),
                            raw_data: data_hex,
                            interpretation: interp.clone(),
                        },
                    };
                    serde_json::to_string_pretty(&output)
                        .map_err(|e| format!("Failed to serialize JSON: {}", e))?
                } else {
                    // Fallback to basic output
                    let output = MemoryDumpOutput {
                        memory_dump: MemoryDumpData {
                            mem_type: dump.mem_type.to_string(),
                            start: format!("0x{:04X}", dump.start_addr),
                            end: format!("0x{:04X}", dump.end_addr),
                            data: data_hex,
                        },
                    };
                    serde_json::to_string_pretty(&output)
                        .map_err(|e| format!("Failed to serialize JSON: {}", e))?
                }
            }
            _ => {
                let output = MemoryDumpOutput {
                    memory_dump: MemoryDumpData {
                        mem_type: dump.mem_type.to_string(),
                        start: format!("0x{:04X}", dump.start_addr),
                        end: format!("0x{:04X}", dump.end_addr),
                        data: data_hex,
                    },
                };
                serde_json::to_string_pretty(&output)
                    .map_err(|e| format!("Failed to serialize JSON: {}", e))?
            }
        };

        Ok(format!("{}\n", json_str).into_bytes())
    }

    fn file_extension(&self) -> &'static str { "json" }
}

/// TOML formatter
pub struct TomlFormatter;

impl MemoryFormatter for TomlFormatter {
    fn format(&self, dump: &MemoryDump) -> Result<Vec<u8>, String> {
        let data_hex: Vec<String> = dump.data.iter().map(|b| format!("0x{:02X}", b)).collect();

        let toml_str = match dump.mem_type {
            MemoryType::Oam => {
                if let Some(ref interp) = dump.interpreted_oam {
                    let output = OamDumpOutput {
                        oam_dump: OamDumpData {
                            mem_type: dump.mem_type.to_string(),
                            size: dump.data.len(),
                            raw_data: data_hex,
                            interpretation: interp.clone(),
                        },
                    };
                    toml::to_string_pretty(&output)
                        .map_err(|e| format!("Failed to serialize TOML: {}", e))?
                } else {
                    let output = MemoryDumpOutput {
                        memory_dump: MemoryDumpData {
                            mem_type: dump.mem_type.to_string(),
                            start: format!("0x{:04X}", dump.start_addr),
                            end: format!("0x{:04X}", dump.end_addr),
                            data: data_hex,
                        },
                    };
                    toml::to_string_pretty(&output)
                        .map_err(|e| format!("Failed to serialize TOML: {}", e))?
                }
            }
            MemoryType::Nametables => {
                if let Some(ref interp) = dump.interpreted_nametables {
                    let output = NametablesDumpOutput {
                        nametables_dump: NametablesDumpData {
                            mem_type: dump.mem_type.to_string(),
                            start: format!("0x{:04X}", dump.start_addr),
                            end: format!("0x{:04X}", dump.end_addr),
                            raw_data: data_hex,
                            interpretation: interp.clone(),
                        },
                    };
                    toml::to_string_pretty(&output)
                        .map_err(|e| format!("Failed to serialize TOML: {}", e))?
                } else {
                    let output = MemoryDumpOutput {
                        memory_dump: MemoryDumpData {
                            mem_type: dump.mem_type.to_string(),
                            start: format!("0x{:04X}", dump.start_addr),
                            end: format!("0x{:04X}", dump.end_addr),
                            data: data_hex,
                        },
                    };
                    toml::to_string_pretty(&output)
                        .map_err(|e| format!("Failed to serialize TOML: {}", e))?
                }
            }
            _ => {
                let output = MemoryDumpOutput {
                    memory_dump: MemoryDumpData {
                        mem_type: dump.mem_type.to_string(),
                        start: format!("0x{:04X}", dump.start_addr),
                        end: format!("0x{:04X}", dump.end_addr),
                        data: data_hex,
                    },
                };
                toml::to_string_pretty(&output)
                    .map_err(|e| format!("Failed to serialize TOML: {}", e))?
            }
        };

        Ok(format!("{}\n", toml_str).into_bytes())
    }

    fn file_extension(&self) -> &'static str { "toml" }
}

// =============================================================================
// OutputFormat Extensions
// =============================================================================

impl OutputFormat {
    /// Get the formatter for this output format.
    ///
    /// To add a new format, add a variant to the enum and a case here.
    pub fn formatter(&self) -> Box<dyn MemoryFormatter> {
        match self {
            OutputFormat::Hex => Box::new(HexFormatter),
            OutputFormat::Json => Box::new(JsonFormatter),
            OutputFormat::Toml => Box::new(TomlFormatter),
            OutputFormat::Binary => Box::new(BinaryFormatter),
        }
    }

    /// Get the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Hex => "hex",
            OutputFormat::Json => "json",
            OutputFormat::Toml => "toml",
            OutputFormat::Binary => "bin",
        }
    }
}

// =============================================================================
// Output Writer
// =============================================================================

/// Track whether the output file has been initialized (created/truncated).
/// Note: This is intentionally global to support multiple OutputWriter
/// instances writing to the same file in append mode within a single CLI run.
static OUTPUT_FILE_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Manages output writing with support for file and stdout.
pub struct OutputWriter {
    /// Output file path (None for stdout)
    path: Option<PathBuf>,
    /// Output format
    format: OutputFormat,
}

impl OutputWriter {
    /// Create a new output writer.
    pub fn new(path: Option<PathBuf>, format: OutputFormat) -> Self {
        Self {
            path,
            format,
        }
    }

    /// Reset the output file state (call at start of output session).
    pub fn reset() { OUTPUT_FILE_INITIALIZED.store(false, Ordering::SeqCst); }

    /// Write a memory dump using the configured format.
    pub fn write(&self, dump: &MemoryDump) -> Result<(), String> {
        let formatter = self.format.formatter();
        let data = formatter.format(dump)?;
        self.write_bytes(&data)
    }

    /// Write raw bytes to the output.
    fn write_bytes(&self, data: &[u8]) -> Result<(), String> {
        let mut writer = self.get_writer()?;
        writer.write_all(data).map_err(|e| e.to_string())
    }

    /// Get the output writer (file or stdout).
    fn get_writer(&self) -> Result<Box<dyn Write>, String> {
        if let Some(ref path) = self.path {
            let is_first_write = !OUTPUT_FILE_INITIALIZED.swap(true, Ordering::SeqCst);

            let file = if is_first_write {
                File::create(path)
            } else {
                OpenOptions::new().append(true).open(path)
            }
            .map_err(|e| format!("Failed to open output file: {}", e))?;

            Ok(Box::new(file))
        } else {
            Ok(Box::new(std::io::stdout()))
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_formatter() {
        let dump = MemoryDump::cpu(0x0000, vec![0x00, 0x01, 0x02, 0x03]);
        let formatter = HexFormatter;
        let output = formatter.format(&dump).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("0000: 00 01 02 03"));
    }

    #[test]
    fn test_binary_formatter() {
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let dump = MemoryDump::cpu(0x1000, data.clone());
        let formatter = BinaryFormatter;
        let output = formatter.format(&dump).unwrap();
        assert_eq!(output, data);
    }

    #[test]
    fn test_json_formatter() {
        let dump = MemoryDump::cpu(0x0000, vec![0xFF]);
        let formatter = JsonFormatter;
        let output = formatter.format(&dump).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("\"type\": \"cpu\""));
        assert!(text.contains("0xFF"));
    }

    #[test]
    fn test_toml_formatter() {
        let dump = MemoryDump::cpu(0x0000, vec![0xFF]);
        let formatter = TomlFormatter;
        let output = formatter.format(&dump).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("type = \"cpu\""));
    }

    #[test]
    fn test_output_format_formatter() {
        assert_eq!(OutputFormat::Hex.extension(), "hex");
        assert_eq!(OutputFormat::Json.extension(), "json");
        assert_eq!(OutputFormat::Toml.extension(), "toml");
        assert_eq!(OutputFormat::Binary.extension(), "bin");
    }

    // =============================================================================
    // OAM Interpretation Tests
    // =============================================================================

    #[test]
    fn test_oam_sprite_from_bytes() {
        // Y=0x20, Tile=0x42, Attr=0b11100011 (pal=3, behind=true, flipH=true,
        // flipV=true), X=0x80
        let bytes = [0x20, 0x42, 0xE3, 0x80];
        let sprite = OamSprite::from_bytes(5, &bytes);

        assert_eq!(sprite.index, 5);
        assert_eq!(sprite.y, 0x20);
        assert_eq!(sprite.tile, 0x42);
        assert_eq!(sprite.x, 0x80);
        assert_eq!(sprite.palette, 3);
        assert!(sprite.behind_background);
        assert!(sprite.flip_h);
        assert!(sprite.flip_v);
        assert!(sprite.visible); // Y < 0xEF
    }

    #[test]
    fn test_oam_sprite_visibility() {
        // Hidden sprite (Y = 0xF0)
        let hidden = OamSprite::from_bytes(0, &[0xF0, 0x00, 0x00, 0x00]);
        assert!(!hidden.visible);

        // Visible sprite (Y = 0xEE)
        let visible = OamSprite::from_bytes(0, &[0xEE, 0x00, 0x00, 0x00]);
        assert!(visible.visible);
    }

    #[test]
    fn test_interpreted_oam_from_raw() {
        // Create full 256-byte OAM data (64 sprites × 4 bytes) with all sprites hidden
        // by default
        let mut data = vec![0xFFu8; 256]; // Y=0xFF means hidden
        // Sprite 0: visible
        data[0] = 0x10; // Y
        data[1] = 0x01; // Tile
        data[2] = 0x00; // Attr
        data[3] = 0x20; // X
        // Sprite 1: explicitly hidden
        data[4] = 0xFF; // Y (hidden)
        data[5] = 0x02; // Tile
        data[6] = 0x00; // Attr
        data[7] = 0x30; // X

        let interp = InterpretedOam::from_raw(&data);

        assert_eq!(interp.sprite_count, 64);
        assert_eq!(interp.visible_count, 1); // Only sprite 0 is visible
        assert_eq!(interp.sprites.len(), 64);
        assert_eq!(interp.sprites[0].y, 0x10);
        assert_eq!(interp.sprites[0].tile, 0x01);
        assert!(interp.sprites[0].visible);
        assert!(!interp.sprites[1].visible);
    }

    #[test]
    fn test_oam_dump_json_has_interpretation() {
        let mut data = vec![0u8; 256];
        data[0] = 0x10; // Y
        data[1] = 0x42; // Tile
        data[2] = 0x01; // Attr (palette 1)
        data[3] = 0x50; // X

        let dump = MemoryDump::oam(data);
        let formatter = JsonFormatter;
        let output = formatter.format(&dump).unwrap();
        let text = String::from_utf8(output).unwrap();

        // Should contain interpretation
        assert!(text.contains("interpretation"));
        assert!(text.contains("sprite_count"));
        assert!(text.contains("visible_count"));
        assert!(text.contains("sprites"));
    }

    #[test]
    fn test_oam_dump_hex_has_interpretation() {
        let mut data = vec![0u8; 256];
        data[0] = 0x10;
        data[1] = 0x42;
        data[2] = 0x01;
        data[3] = 0x50;

        let dump = MemoryDump::oam(data);
        let formatter = HexFormatter;
        let output = formatter.format(&dump).unwrap();
        let text = String::from_utf8(output).unwrap();

        // Should contain interpretation header
        assert!(text.contains("=== OAM Interpretation ==="));
        assert!(text.contains("Total sprites:"));
        assert!(text.contains("Visible:"));
        assert!(text.contains("=== Raw OAM Data ==="));
    }

    // =============================================================================
    // Nametable Interpretation Tests
    // =============================================================================

    #[test]
    fn test_interpreted_nametable_from_raw() {
        // Create 1KB nametable data
        let mut data = vec![0u8; 1024];
        // Set some tile values
        data[0] = 0x01; // Row 0, Col 0
        data[31] = 0x1F; // Row 0, Col 31
        data[32] = 0x20; // Row 1, Col 0
        // Attribute table starts at 0x3C0
        data[0x3C0] = 0b11_10_01_00; // Different palettes for 4 quadrants

        let nt = InterpretedNametable::from_raw(0, &data);

        assert_eq!(nt.index, 0);
        assert_eq!(nt.base_address, "0x2000");
        assert_eq!(nt.tiles.len(), 30);
        assert_eq!(nt.tiles[0][0], 0x01);
        assert_eq!(nt.tiles[0][31], 0x1F);
        assert_eq!(nt.tiles[1][0], 0x20);
        assert_eq!(nt.attributes.len(), 64);
        assert_eq!(nt.attributes[0], 0b11_10_01_00);

        // Check palette calculation for first attribute byte
        assert_eq!(nt.tile_palettes[0][0], 0); // Top-left quadrant
        assert_eq!(nt.tile_palettes[0][2], 1); // Top-right quadrant
        assert_eq!(nt.tile_palettes[2][0], 2); // Bottom-left quadrant
        assert_eq!(nt.tile_palettes[2][2], 3); // Bottom-right quadrant
    }

    #[test]
    fn test_interpreted_nametables_from_raw() {
        // Create 4KB data for all 4 nametables
        let data = vec![0u8; 4096];

        let interp = InterpretedNametables::from_raw(&data);

        assert_eq!(interp.total_size, 4096);
        assert_eq!(interp.nametables.len(), 4);
        assert_eq!(interp.nametables[0].base_address, "0x2000");
        assert_eq!(interp.nametables[1].base_address, "0x2400");
        assert_eq!(interp.nametables[2].base_address, "0x2800");
        assert_eq!(interp.nametables[3].base_address, "0x2C00");
    }

    #[test]
    fn test_nametables_dump_json_has_interpretation() {
        let data = vec![0u8; 4096];
        let dump = MemoryDump::nametables(data);
        let formatter = JsonFormatter;
        let output = formatter.format(&dump).unwrap();
        let text = String::from_utf8(output).unwrap();

        // Should contain interpretation
        assert!(text.contains("interpretation"));
        assert!(text.contains("total_size"));
        assert!(text.contains("nametables"));
        assert!(text.contains("base_address"));
    }

    #[test]
    fn test_nametables_dump_hex_has_interpretation() {
        let data = vec![0u8; 4096];
        let dump = MemoryDump::nametables(data);
        let formatter = HexFormatter;
        let output = formatter.format(&dump).unwrap();
        let text = String::from_utf8(output).unwrap();

        // Should contain interpretation header
        assert!(text.contains("=== Nametables Interpretation ==="));
        assert!(text.contains("Nametable 0"));
        assert!(text.contains("=== Raw Nametable Data ==="));
    }
}
