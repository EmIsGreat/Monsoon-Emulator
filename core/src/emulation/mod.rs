//! NES emulation components.
//!
//! This module contains all emulation subsystems for the Nintendo Entertainment System.
//!
//! # User-Facing Modules
//!
//! - [`nes`] — The main [`Nes`](nes::Nes) emulator and execution control.
//! - [`rom`] — ROM file parsing and the builder API.
//! - [`savestate`] — Serializable emulator state snapshots.
//! - [`screen_renderer`] — Trait and types for custom pixel rendering.
//! - [`palette_util`] — NES color palette types and parsing.
//!
//! # Internal Modules
//!
//! The following modules expose hardware implementation details. They are public
//! for advanced use and workspace-internal access, but most library consumers
//! should not depend on them directly:
//!
//! - [`cpu`] — MOS 6502 CPU emulation internals.
//! - [`ppu`] — 2C02 PPU emulation internals and debug data types.
//! - [`mem`] — Memory subsystem (RAM, ROM, memory maps, I/O registers).
//! - [`opcode`] — 6502 opcode definitions and lookup tables.

#[doc(hidden)]
pub mod cpu;
#[doc(hidden)]
pub mod mem;
pub mod nes;
#[doc(hidden)]
pub mod opcode;
pub mod palette_util;
#[doc(hidden)]
pub mod ppu;
pub mod rom;
pub mod savestate;
pub mod screen_renderer;
