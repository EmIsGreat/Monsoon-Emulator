//! Utility traits and functions.
//!
//! This module provides serialization helpers ([`ToBytes`]), hash utilities
//! ([`Hashable`], [`compute_hash`]), and low-level address arithmetic used
//! internally by the CPU emulation.

use crate::emulation::cpu::UPPER_BYTE;
use crate::emulation::mem::{Memory, MemoryDevice};
use crate::emulation::savestate::{BINARY_FORMAT_VERSION, JSON_FORMAT_VERSION, MAGIC, SaveState};

/// Returns `true` if adding `offset` to `base` crosses a 256-byte page boundary.
///
/// This is used by the 6502 CPU to detect page crossings for addressing
/// modes that add an unsigned offset.
#[inline(always)]
pub fn crosses_page_boundary_u8(base: u16, offset: u8) -> bool {
    (base & UPPER_BYTE) != ((base + offset as u16) & UPPER_BYTE)
}

/// Returns `true` if adding a signed `offset` to `base` crosses a 256-byte page boundary.
///
/// This is used by the 6502 CPU for relative branch offset calculations.
#[inline(always)]
pub fn crosses_page_boundary_i8(base: u16, offset: i8) -> bool {
    let target = base.wrapping_add(offset as i16 as u16);
    (base & UPPER_BYTE) != (target & UPPER_BYTE)
}

/// Adds `add` to only the low byte of `val`, preserving the high byte.
///
/// This emulates the 6502 bug where some addressing modes wrap within
/// a page instead of crossing into the next page.
#[inline(always)]
pub fn add_to_low_byte(val: u16, add: u8) -> u16 {
    let high = val & 0xFF00; // preserve high byte
    let low = ((val & 0x00FF) as u8).wrapping_add(add); // add with wrapping
    high | low as u16
}

/// Trait for types that can produce a fast, non-cryptographic hash.
///
/// Used for change detection (e.g., detecting when palette data has been
/// modified) rather than for security purposes.
pub trait Hashable {
    /// Computes a 64-bit FNV-1a hash of this value.
    fn hash(&self) -> u64;
}

/// Trait for types that can be serialized to a byte vector.
///
/// The optional `format` parameter selects the encoding:
/// - `None` or `Some("binary")` — compact binary format (postcard).
/// - `Some("json")` — human-readable JSON format.
pub trait ToBytes {
    /// Serializes this value to bytes in the specified format.
    fn to_bytes(&self, format: Option<String>) -> Vec<u8>;
}

impl ToBytes for SaveState {
    fn to_bytes(&self, format: Option<String>) -> Vec<u8> {
        let mut res = Vec::new();

        res.extend(MAGIC);
        let format = if let Some(format) = format {
            format
        } else {
            "binary".to_string()
        };

        if format == "json" {
            res.push(JSON_FORMAT_VERSION);
            res.extend(serde_json::to_vec_pretty(self).expect("Error serializing SaveState"));
        } else {
            res.push(BINARY_FORMAT_VERSION);
            res.extend(postcard::to_stdvec(self).expect("Error serializing SaveState"));
        }

        res
    }
}

impl Hashable for Memory {
    fn hash(&self) -> u64 { compute_hash(&self.snapshot_all()) }
}

impl Hashable for Vec<u8> {
    fn hash(&self) -> u64 { compute_hash(self) }
}

/// Compute a fast hash of the given data for change detection.
/// Uses FNV-1a algorithm which is fast and has good distribution.
#[inline]
pub fn compute_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xCBF29CE484222325;
    const FNV_PRIME: u64 = 0x100000001B3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
