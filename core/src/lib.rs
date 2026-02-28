//! # Monsoon Core
//!
//! `monsoon_core` is the core emulation library for the Monsoon NES emulator.
//! It provides a cycle-accurate emulation of the Nintendo Entertainment System (NES),
//! including the MOS 6502 CPU, the 2C02 PPU (Picture Processing Unit), memory subsystems,
//! and ROM parsing for the iNES and NES 2.0 file formats.
//!
//! ## Quick Start
//!
//! The main entry point is the [`Nes`] struct, which orchestrates all emulation components:
//!
//! ```rust,no_run
//! use monsoon_core::emulation::nes::Nes;
//! use monsoon_core::emulation::rom::RomFile;
//!
//! let mut nes = Nes::default();
//!
//! // Load a ROM from raw bytes
//! let rom_data = std::fs::read("game.nes").unwrap();
//! let rom = RomFile::load(&rom_data, None).unwrap();
//! nes.load_rom(&rom);
//!
//! // Power on and run a single frame
//! nes.power();
//! nes.step_frame().expect("emulation error");
//!
//! // Get the rendered frame as a buffer of palette indices
//! let pixels = nes.get_pixel_buffer();
//! ```
//!
//! ## Architecture
//!
//! The library is organized into the following public modules:
//!
//! - [`emulation::nes`] — The top-level [`Nes`] emulator struct and execution control.
//! - [`emulation::rom`] — ROM file parsing ([`RomFile`]) and the [`RomBuilder`] for
//!   constructing ROM metadata programmatically.
//! - [`emulation::savestate`] — Save and restore emulator state via [`SaveState`].
//! - [`emulation::screen_renderer`] — The [`ScreenRenderer`] trait for implementing
//!   custom pixel renderers, plus a built-in [`NoneRenderer`].
//! - [`emulation::palette_util`] — NES color palette types ([`RgbColor`], [`RgbPalette`])
//!   and palette file parsing.
//! - [`emulation::ppu_util`] — PPU constants and debug data types (e.g., output dimensions,
//!   [`EmulatorFetchable`](emulation::ppu_util::EmulatorFetchable) for debug views).
//! - [`util`] — Serialization helpers ([`ToBytes`]) and hash utilities.
//!
//! Internal implementation modules (`cpu`, `ppu`, `mem`, `opcode`) are `pub(crate)` and not
//! accessible to downstream consumers.
//!
//! ## Pixel Buffer Format
//!
//! [`Nes::get_pixel_buffer`] returns a `Vec<u16>` of palette indices, **not** RGB values.
//! Each entry encodes:
//!
//! - **Bits 0-5**: NES color index (0-63)
//! - **Bits 6-8**: Emphasis bits from the PPU mask register
//!
//! Use a [`ScreenRenderer`] implementation (e.g., `LookupPaletteRenderer` from the
//! `monsoon-default-renderers` crate) to convert these indices to RGB colors.
//!
//! ## Save States
//!
//! The emulator supports serializable save states through the [`SaveState`] type.
//! States can be serialized to binary (postcard) or JSON format using the [`ToBytes`]
//! trait, and deserialized with [`try_load_state_from_bytes`].
//!
//! [`Nes`]: emulation::nes::Nes
//! [`Nes::get_pixel_buffer`]: emulation::nes::Nes::get_pixel_buffer
//! [`RomFile`]: emulation::rom::RomFile
//! [`RomBuilder`]: emulation::rom::RomBuilder
//! [`SaveState`]: emulation::savestate::SaveState
//! [`ScreenRenderer`]: emulation::screen_renderer::ScreenRenderer
//! [`NoneRenderer`]: emulation::screen_renderer::NoneRenderer
//! [`RgbColor`]: emulation::palette_util::RgbColor
//! [`RgbPalette`]: emulation::palette_util::RgbPalette
//! [`ToBytes`]: util::ToBytes
//! [`try_load_state_from_bytes`]: emulation::savestate::try_load_state_from_bytes

#![feature(ascii_char)]
extern crate core;
pub mod emulation;
#[cfg(test)]
mod tests;
mod trace;
pub mod util;
