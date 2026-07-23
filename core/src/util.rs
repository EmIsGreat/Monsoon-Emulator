//! Utility traits and functions.
//!
//! This module provides serialization helpers ([`ToBytes`]) and hash utilities
//! ([`Hashable`]) for use by the emulator and consumers of this library.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::emulation::cpu::UPPER_BYTE;
use crate::emulation::mem::Memory;
use crate::emulation::savestate::{BINARY_FORMAT_VERSION, JSON_FORMAT_VERSION, MAGIC, SaveState};
/// Returns `true` if adding a signed `offset` to `base` crosses a 256-byte page
/// boundary.
///
/// This is used by the 6502 CPU for relative branch offset calculations.
#[inline]
pub(crate) fn crosses_page_boundary_i8(base: u16, offset: i8) -> bool {
    let target = base.wrapping_add_signed(i16::from(offset));
    (base & UPPER_BYTE) != (target & UPPER_BYTE)
}

/// Adds `add` to only the low byte of `val`, preserving the high byte.
///
/// This emulates the 6502 bug where some addressing modes wrap within
/// a page instead of crossing into the next page.
#[inline]
pub(crate) fn add_to_low_byte(val: u16, add: u8) -> u16 {
    let high = val & 0xFF00; // preserve high byte
    let low = ((val & 0x00FF) as u8).wrapping_add(add); // add with wrapping
    high | u16::from(low)
}

#[derive(Debug)]
pub enum HashError {
    SerializationError(SerializationError),
}

impl Display for HashError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Error hashing data")?;
        match self {
            HashError::SerializationError(err) => {
                write!(f, "{err}")
            }
        }
    }
}

impl Error for HashError {}

impl From<SerializationError> for HashError {
    fn from(err: SerializationError) -> Self { HashError::SerializationError(err) }
}

/// Trait for types that can produce a fast, non-cryptographic hash.
///
/// Used for change detection (e.g., detecting when palette data has been
/// modified) rather than for security purposes.
pub trait Hashable {
    /// Computes a 64-bit FNV-1a hash of this value.
    /// # Errors
    /// Returns `err` if the passed value could not be hashed
    fn hash(&self) -> Result<u64, HashError>;
}

#[derive(Debug)]
pub enum SerializationError {
    SerdeJsonError(serde_json::Error),
    PostcardError(postcard::Error),
}

impl Display for SerializationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializationError::SerdeJsonError(err) => {
                f.write_str("Error serializing data to json")?;
                write!(f, "{err}")
            }
            SerializationError::PostcardError(err) => {
                f.write_str("Error serializing data using postcard")?;
                write!(f, "{err}")
            }
        }
    }
}

impl Error for SerializationError {}

impl From<serde_json::Error> for SerializationError {
    fn from(err: serde_json::Error) -> Self { SerializationError::SerdeJsonError(err) }
}

impl From<postcard::Error> for SerializationError {
    fn from(err: postcard::Error) -> Self { SerializationError::PostcardError(err) }
}

/// Trait for types that can be serialized to a byte vector.
///
/// The optional `format` parameter selects the encoding:
/// - `None` or `Some("binary")` — compact binary format (postcard).
/// - `Some("json")` — human-readable JSON format.
pub trait ToBytes {
    /// Serializes this value to bytes in the specified format.
    /// # Errors
    /// Returns `err` if the passed value could not be hashed
    fn to_bytes(&self, format: Option<String>) -> Result<Vec<u8>, SerializationError>;
}

impl ToBytes for SaveState {
    fn to_bytes(&self, format: Option<String>) -> Result<Vec<u8>, SerializationError> {
        let mut res = Vec::new();

        res.extend(MAGIC);
        let format = if let Some(format) = format {
            format
        } else {
            "binary".to_string()
        };

        if format == "json" {
            res.push(JSON_FORMAT_VERSION);

            res.extend(serde_json::to_vec_pretty(self)?);
        } else {
            res.push(BINARY_FORMAT_VERSION);
            res.extend(postcard::to_stdvec(self)?);
        }

        Ok(res)
    }
}

impl Hashable for Memory {
    fn hash(&self) -> Result<u64, HashError> {
        let mut base = self.snapshot_all();
        base.push(self.is_write.into());
        Ok(compute_hash(&base[..]))
    }
}

impl Hashable for Vec<u8> {
    fn hash(&self) -> Result<u64, HashError> { Ok(compute_hash(self)) }
}

/// Compute a fast hash of the given data for change detection.
/// Uses FNV-1a algorithm which is fast and has good distribution.
#[inline]
pub(crate) fn compute_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xCBF2_9CE4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01B3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[must_use]
pub fn format_bytes_human_readable(bytes: u32) -> String {
    const UNITS: [&str; 3] = ["Bytes", "KB", "MB"];

    let mut value = f64::from(bytes);
    let mut unit_idx = 0usize;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{bytes} {}", UNITS[unit_idx])
    } else {
        format!("{value:.2} {} ({bytes} Bytes)", UNITS[unit_idx])
    }
}

/// Parse a hexadecimal u16 value (with or without 0x prefix)
/// # Errors
/// Returns `err` if the passed value is not a valid hex number
pub fn parse_hex_u16(s: &str) -> Result<u16, String> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u16::from_str_radix(s, 16).map_err(|e| format!("Invalid hex value '{s}': {e}"))
}

/// Parse a hexadecimal u8 value (with or without 0x prefix)
/// # Errors
/// Returns `err` if the passed value is not a valid hex number
pub fn parse_hex_u8(s: &str) -> Result<u8, String> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u8::from_str_radix(s, 16).map_err(|e| format!("Invalid hex value '{s}': {e}"))
}

/// Parse a hex string to u16, returning None on failure
#[must_use]
pub fn parse_hex_u16_opt(s: &str) -> Option<u16> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u16::from_str_radix(s, 16).ok()
}

/// Parse a hex string to u8, returning None on failure
#[must_use]
pub fn parse_hex_u8_opt(s: &str) -> Option<u8> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u8::from_str_radix(s, 16).ok()
}

#[macro_export]
macro_rules! cpu_bus_view {
    ($self:expr) => {
        CpuBusView::from(
            &mut $self.board.mapper,
            &mut $self.board.cpu_open_bus,
            &mut $self.board.ppu_open_bus,
            &mut $self.board.cpu_ram,
            &mut $self.board.nametable_ram,
            &mut $self.board.palette_ram,
            &mut $self.board.ppu,
            &mut $self.board.apu,
            &mut $self.board.irq,
            &mut $self.board.controller1,
            &mut $self.board.controller2,
            &mut $self.board.joystick_strobe_data,
        )
    };
}

#[macro_export]
macro_rules! ppu_bus_view {
    ($self:expr, $grayscale:expr) => {
        PpuBusView::from(
            &mut $self.board.mapper,
            &mut $self.board.ppu_open_bus,
            &mut $self.board.nametable_ram,
            &mut $self.board.palette_ram,
            $grayscale,
        )
    };
}
