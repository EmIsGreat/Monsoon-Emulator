//! Save state serialization and deserialization.
//!
//! This module provides types for capturing and restoring the full emulator
//! state. Save states can be serialized to a compact binary format (postcard)
//! or a human-readable JSON format using the [`ToBytes`](crate::util::ToBytes) trait,
//! and deserialized with [`try_load_state_from_bytes()`].
//!
//! # Wire Format
//!
//! Serialized save states start with a 5-byte magic header (`ESSV1`), followed
//! by a 1-byte format version (`0` = binary/postcard, `1` = JSON), then the
//! payload.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::emulation::cpu::{Cpu, INTERNAL_RAM_SIZE, MicroOp};
use crate::emulation::mem::OpenBus;
use crate::emulation::ppu::{Ppu, VRAM_SIZE};
use crate::emulation::rom::RomFile;

/// Magic header bytes identifying a Monsoon save state file (`"ESSV1"`).
pub const MAGIC: &[u8; 5] = b"ESSV1"; // NES SaveState
/// Format version byte for binary (postcard) encoding.
pub const BINARY_FORMAT_VERSION: u8 = 0;
/// Format version byte for JSON encoding.
pub const JSON_FORMAT_VERSION: u8 = 1;

/// Snapshot of the CPU state at a specific point in time.
///
/// All 6502 registers, internal RAM, PRG RAM, and micro-operation state
/// are captured. This is part of a [`SaveState`] and is not typically
/// constructed directly by library users.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CpuState {
    /// 16-bit program counter.
    pub program_counter: u16,
    /// 8-bit stack pointer (offset from `$0100`).
    pub stack_pointer: u8,
    /// Accumulator register.
    pub accumulator: u8,
    /// X index register.
    pub x_register: u8,
    /// Y index register.
    pub y_register: u8,
    /// Processor status flags (NV-BDIZC).
    pub processor_status: u8,
    /// Internal RAM snapshot (2 KB, addresses `$0000`-`$07FF`).
    pub internal_ram: Vec<u8>,
    /// PRG RAM snapshot if present (up to 8 KB, addresses `$6000`-`$7FFF`).
    pub prg_ram: Vec<u8>,
    /// Full memory dump for tracing/debug (not serialized to reduce size).
    #[serde(skip)]
    pub memory: Vec<u8>,
    /// Low byte of the current address being assembled.
    pub(crate) lo: u8,
    /// High byte of the current address being assembled.
    pub(crate) hi: u8,
    /// Current micro-operation being executed.
    pub(crate) current_op: MicroOp,
    /// Queue of pending micro-operations.
    pub(crate) op_queue: VecDeque<MicroOp>,
    /// Opcode byte of the instruction currently being executed.
    pub(crate) current_opcode: Option<u8>,
    /// Temporary register for intermediate calculations.
    pub(crate) temp: u8,
    /// Constant used by the ANE (XAA) illegal opcode.
    pub(crate) ane_constant: u8,
    /// Whether the CPU has executed a halt (KIL) instruction.
    pub is_halted: bool,
    /// Whether the current cycle is a read cycle.
    pub(crate) read_cycle: bool,
    /// IRQ detection flag.
    pub(crate) irq_detected: bool,
    /// IRQ pending flag.
    pub(crate) irq_pending: bool,
    /// External IRQ line state.
    pub(crate) irq_provider: bool,
    /// Whether the CPU is currently in an IRQ handler.
    pub(crate) is_in_irq: bool,
    /// Current interrupt vector address.
    pub(crate) current_irq_vec: u16,
    /// Locked interrupt vector address.
    pub(crate) locked_irq_vec: u16,
    /// DMA source page register.
    pub(crate) dma_page: u8,
    /// DMA read/write toggle.
    pub(crate) dma_read: bool,
    /// DMA temporary data register.
    pub(crate) dma_temp: u8,
    /// Whether a DMA transfer has been triggered.
    pub(crate) dma_triggered: bool,
    /// NMI detection flag.
    pub(crate) nmi_detected: bool,
    /// NMI pending flag.
    pub(crate) nmi_pending: bool,
    /// Previous NMI line state (for edge detection).
    pub(crate) prev_nmi: bool,
}

impl From<&Cpu> for CpuState {
    fn from(cpu: &Cpu) -> Self {
        Self {
            program_counter: cpu.program_counter,
            stack_pointer: cpu.stack_pointer,
            accumulator: cpu.accumulator,
            x_register: cpu.x_register,
            y_register: cpu.y_register,
            processor_status: cpu.processor_status,
            // Only save internal RAM (2KB) - addresses 0x0000-0x07FF
            internal_ram: cpu
                .memory
                .get_memory_debug(Some(0x0..=(INTERNAL_RAM_SIZE - 1))),
            // Save PRG RAM if present (0x6000-0x7FFF)
            prg_ram: cpu.memory.get_memory_debug(Some(0x6000..=0x7FFF)),
            // Full memory dump for tracing (not serialized)
            memory: cpu.memory.get_memory_debug(Some(0x0..=0xFFFF)),
            lo: cpu.lo,
            hi: cpu.hi,
            current_op: cpu.current_op,
            op_queue: cpu.op_queue.clone(),
            current_opcode: cpu.current_opcode.map(|c| c.opcode),
            temp: cpu.temp,
            ane_constant: cpu.ane_constant,
            is_halted: cpu.is_halted,
            read_cycle: cpu.cpu_read_cycle,
            irq_detected: cpu.irq_detected,
            irq_pending: cpu.irq_pending,
            irq_provider: cpu.irq_provider.get(),
            is_in_irq: cpu.is_in_irq,
            current_irq_vec: cpu.current_irq_vec,
            locked_irq_vec: cpu.locked_irq_vec,
            dma_page: cpu.dma_page,
            dma_read: cpu.dma_read,
            dma_temp: cpu.dma_temp,
            dma_triggered: cpu.dma_triggered,
            nmi_detected: cpu.nmi_detected,
            nmi_pending: cpu.nmi_pending,
            prev_nmi: cpu.prev_nmi,
        }
    }
}

/// Snapshot of the PPU state at a specific point in time.
///
/// Contains all PPU registers, VRAM, OAM, palette RAM, and internal
/// rendering state. This is part of a [`SaveState`] and is not typically
/// constructed directly by library users.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PpuState {
    /// Total dot cycles elapsed.
    pub cycle_counter: u128,
    /// Counter for VBL clear scheduling.
    pub(crate) vbl_reset_counter: u8,
    /// PPU status register (`$2002`).
    pub status_register: u8,
    /// PPU control register (`$2000`).
    pub ctrl_register: u8,
    /// PPU mask register (`$2001`).
    pub mask_register: u8,
    /// Whether an NMI has been requested.
    pub nmi_requested: bool,
    /// Nametable VRAM snapshot (2 KB).
    pub nametable_ram: Vec<u8>,
    /// Current VRAM address register (v).
    pub ppu_addr_register: u16,
    /// OAM address register (`$2003`).
    pub oam_addr_register: u8,
    /// Write latch state (first/second write toggle).
    pub(crate) write_latch: bool,
    /// PPU data read buffer.
    pub(crate) ppu_data_buffer: u8,
    /// Temporary VRAM address register (t).
    pub(crate) t_register: u16,
    /// Next background tile attribute byte.
    pub(crate) bg_next_tile_attribute: u8,
    /// Fine X scroll (0-7).
    pub fine_x_scroll: u8,
    /// Whether the current frame is an even frame.
    pub even_frame: bool,
    /// Reset signal state.
    pub(crate) reset_signal: bool,
    /// Current dot position within the scanline (0-340).
    pub dot: u16,
    /// Current scanline (0-261).
    pub scanline: u16,
    /// Next background tile ID.
    pub(crate) bg_next_tile_id: u8,
    /// Next background tile pattern low byte.
    pub(crate) bg_next_tile_lsb: u8,
    /// Scheduled VBL clear timing.
    pub(crate) vbl_clear_scheduled: Option<u8>,
    /// Previous VBL state for edge detection.
    pub(crate) prev_vbl: u8,
    /// Open bus state.
    pub(crate) open_bus: OpenBus,
    /// Current address on the PPU address bus.
    pub(crate) address_bus: u16,
    /// Address latch value.
    pub(crate) address_latch: u8,
    /// Background shift register (pattern low).
    pub(crate) shift_pattern_lo: u16,
    /// Background shift register (pattern high).
    pub(crate) shift_pattern_hi: u16,
    /// Background shift register (attribute low).
    pub(crate) shift_attr_lo: u8,
    /// Background shift register (attribute high).
    pub(crate) shift_attr_hi: u8,
    /// Attribute shift-in latch (low).
    pub(crate) shift_in_attr_lo: bool,
    /// Attribute shift-in latch (high).
    pub(crate) shift_in_attr_hi: bool,
    /// Whether secondary OAM clear is active.
    pub(crate) is_soam_clear_active: bool,
    /// Primary OAM evaluation index.
    pub(crate) oam_index: u8,
    /// Secondary OAM write index.
    pub(crate) soam_index: u8,
    /// Secondary OAM write disable flag.
    pub(crate) soam_disable: bool,
    /// OAM byte increment counter.
    pub(crate) oam_increment: u8,
    /// Secondary OAM write counter.
    pub(crate) soam_write_counter: u8,
    /// OAM fetch data register.
    pub(crate) oam_fetch: u8,
    /// OAM (sprite) memory snapshot (256 bytes).
    pub oam_mem: Vec<u8>,
    /// Palette RAM snapshot (32 bytes).
    pub palette_ram: Vec<u8>,
}

impl From<&Ppu> for PpuState {
    fn from(ppu: &Ppu) -> Self {
        Self {
            cycle_counter: ppu.dot_counter,
            vbl_reset_counter: ppu.vbl_reset_counter.get(),
            status_register: ppu.status_register.get(),
            ctrl_register: ppu.ctrl_register,
            mask_register: ppu.mask_register,
            nmi_requested: ppu.nmi_requested.get(),
            // Only save nametable VRAM (2KB) - addresses 0x2000-0x27FF
            nametable_ram: ppu
                .memory
                .get_memory_debug(Some(0x2000..=(0x2000 + (VRAM_SIZE as u16 * 2)))),
            ppu_addr_register: ppu.v_register,
            oam_addr_register: ppu.oam_addr_register,
            write_latch: ppu.write_latch.get(),
            ppu_data_buffer: ppu.ppu_data_buffer,
            t_register: ppu.t_register,
            bg_next_tile_id: ppu.bg_next_tile_id,
            bg_next_tile_lsb: ppu.bg_next_tile_lsb,
            vbl_clear_scheduled: ppu.vbl_clear_scheduled.get(),
            prev_vbl: ppu.prev_vbl,
            open_bus: ppu.open_bus.get(),
            address_bus: ppu.address_bus,
            address_latch: ppu.address_latch,
            shift_pattern_lo: ppu.shift_pattern_lo,
            shift_pattern_hi: ppu.shift_pattern_hi,
            shift_attr_lo: ppu.shift_attr_lo,
            shift_attr_hi: ppu.shift_attr_hi,
            shift_in_attr_lo: ppu.shift_in_attr_lo,
            shift_in_attr_hi: ppu.shift_in_attr_hi,
            is_soam_clear_active: ppu.is_soam_clear_active,
            oam_index: ppu.oam_index,
            soam_index: ppu.soam_index,
            soam_disable: ppu.soam_disable,
            oam_increment: ppu.oam_increment,
            soam_write_counter: ppu.soam_write_counter,
            oam_fetch: ppu.oam_fetch,
            oam_mem: ppu.oam.get_memory_debug(None),
            bg_next_tile_attribute: ppu.bg_next_tile_attribute,
            fine_x_scroll: ppu.fine_x_scroll,
            even_frame: ppu.even_frame,
            reset_signal: ppu.reset_signal,
            dot: ppu.dot,
            scanline: ppu.scanline,
            palette_ram: ppu.palette_ram.get_memory_debug(None),
        }
    }
}

/// A complete snapshot of the NES emulator state.
///
/// Contains the CPU state, PPU state, loaded ROM metadata, and timing
/// information. Can be serialized with [`ToBytes::to_bytes()`](crate::util::ToBytes::to_bytes)
/// and deserialized with [`try_load_state_from_bytes()`].
///
/// # Serialization
///
/// ```rust,no_run
/// use monsoon_core::util::ToBytes;
/// # use monsoon_core::emulation::savestate::SaveState;
///
/// # fn example(state: SaveState) {
/// // Binary format (compact, fast)
/// let bytes = state.to_bytes(None);
///
/// // JSON format (human-readable, larger)
/// let json_bytes = state.to_bytes(Some("json".to_string()));
/// # }
/// ```
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SaveState {
    /// Captured CPU state.
    pub cpu: CpuState,
    /// Captured PPU state.
    pub ppu: PpuState,
    /// ROM metadata (raw data is skipped in serialization).
    pub rom_file: RomFile,
    /// Save state format version.
    pub version: u16,
    /// Total master clock cycles at the time of capture.
    pub total_cycles: u128,
    /// CPU clock divider counter at the time of capture.
    pub cycle: u8,
    /// PPU clock divider counter at the time of capture.
    pub ppu_cycle_counter: u8,
    /// CPU clock divider counter at the time of capture.
    pub cpu_cycle_counter: u8,
}

/// Attempts to deserialize a [`SaveState`] from raw bytes.
///
/// Returns `None` if the bytes do not contain a valid save state
/// (wrong magic header, unsupported format version, or corrupted data).
///
/// # Wire Format
///
/// The expected byte layout is:
/// 1. 5-byte magic header: `ESSV1`
/// 2. 1-byte format version: `0` (binary) or `1` (JSON)
/// 3. Payload (postcard or JSON encoded [`SaveState`])
///
/// # Example
///
/// ```rust,no_run
/// use monsoon_core::emulation::savestate::try_load_state_from_bytes;
///
/// # let bytes: &[u8] = &[];
/// if let Some(state) = try_load_state_from_bytes(bytes) {
///     println!("Loaded state at cycle {}", state.total_cycles);
/// }
/// ```
pub fn try_load_state_from_bytes(encoded: &[u8]) -> Option<SaveState> {
    if encoded.len() < MAGIC.len() + 1 {
        return None;
    }

    if &encoded[..MAGIC.len()] != MAGIC {
        return None;
    }

    let format = encoded[MAGIC.len()];
    let payload = &encoded[MAGIC.len() + 1..];

    match format {
        JSON_FORMAT_VERSION => serde_json::from_slice(payload).ok(),
        BINARY_FORMAT_VERSION => postcard::from_bytes(payload).ok(),
        _ => None,
    }
}
