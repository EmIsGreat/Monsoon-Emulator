//! NES emulation components.
//!
//! This module contains all emulation subsystems for the Nintendo Entertainment
//! System.
//!
//! # Public Modules
//!
//! - [`nes`] — The main [`Nes`](nes::Nes) emulator and execution control.
//! - [`rom`] — ROM file parsing and the builder API.
//! - [`savestate`] — Serializable emulator state snapshots.
//! - [`screen_renderer`] — Trait and types for custom pixel rendering.
//! - [`palette_util`] — NES color palette types and parsing.
//! - [`ppu_util`] — PPU constants and debug data types used by frontends.
//!
//! # Internal Modules
//!
//! The following modules contain hardware implementation details. They are
//! `pub(crate)` and not accessible to downstream consumers:
//!
//! - `cpu` — MOS 6502 CPU emulation internals.
//! - `ppu` — 2C02 PPU emulation internals.
//! - `mem` — Memory subsystem (RAM, ROM, memory maps, I/O registers).
//! - `opcode` — 6502 opcode definitions and lookup tables.

pub(crate) mod cpu;
pub(crate) mod mem;
pub mod nes;
pub(crate) mod opcode;
pub mod palette_util;
pub(crate) mod ppu;
pub mod ppu_util;
pub mod rom;
pub mod savestate;
pub mod screen_renderer;
