//! Save state serialization and deserialization.
//!
//! This module provides types for capturing and restoring the full emulator
//! state. Save states can be serialized to a compact binary format (postcard)
//! or a human-readable JSON format using the [`ToBytes`](crate::util::ToBytes)
//! trait, and deserialized with [`try_load_state_from_bytes()`].
//!
//! # Wire Format
//!
//! Serialized save states start with a 5-byte magic header (`ESSV1`), followed
//! by a 1-byte format version (`0` = binary/postcard, `1` = JSON), then the
//! payload.

use serde::{Deserialize, Serialize};
use static_assertions::assert_impl_all;

use crate::emulation::apu::{Apu, FrameCounter};
use crate::emulation::board::Board;
use crate::emulation::cpu::{Cpu, DmaState, IRQState, MicroOp, NMIState, OpQueue};
use crate::emulation::mapper::Mapper;
use crate::emulation::mem::palette_ram::PaletteRam;
use crate::emulation::mem::{Memory, OpenBus};
use crate::emulation::opcode;
use crate::emulation::opcode::{OPCODES_TABLE, get_opcode};
use crate::emulation::peripherals::{Peripheral, StandardController};
use crate::emulation::ppu::{Ppu, SpriteFifo, TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH};
use crate::emulation::rom::RomFile;

/// Magic header bytes identifying a Monsoon save state file (`"ESSV1"`).
pub const MAGIC: &[u8; 5] = b"ESSV1"; // NES SaveState
/// Format version byte for binary (postcard) encoding.
pub const BINARY_FORMAT_VERSION: u8 = 0;
/// Format version byte for JSON encoding.
pub const JSON_FORMAT_VERSION: u8 = 1;

pub const VERSION: u16 = 2;

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
    /// Low byte of the current address being assembled.
    pub(crate) lo: u8,
    /// High byte of the current address being assembled.
    pub(crate) hi: u8,
    /// Current micro-operation being executed.
    pub(crate) current_op: MicroOp,
    /// Queue of pending micro-operations.
    pub(crate) op_queue: OpQueue<8>,
    /// Opcode byte of the instruction currently being executed.
    pub(crate) current_opcode: u8,
    /// CPU data bus / last-fetched data byte.
    pub(crate) data_bus: u8,
    /// Constant used by the ANE (XAA) illegal opcode.
    pub(crate) ane_constant: u8,
    /// Whether the CPU has executed a halt (KIL) instruction.
    pub is_halted: bool,
    pub(crate) dma_state: DmaState,
    pub(crate) nmi_state: NMIState,
    pub(crate) irq_state: IRQState,
    pub cycle: u64,
    pub remaining_dma_cycles: u16,
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
            lo: cpu.lo,
            hi: cpu.hi,
            current_op: cpu.current_op,
            op_queue: cpu.op_queue,
            current_opcode: cpu.current_opcode.opcode,
            data_bus: cpu.data_bus,
            ane_constant: cpu.ane_constant,
            is_halted: cpu.is_halted,
            irq_state: cpu.irq_state,
            dma_state: cpu.dma_state,
            nmi_state: cpu.nmi_state,
            cycle: cpu.cycle,
            remaining_dma_cycles: cpu.remaining_dma_cycles,
        }
    }
}

impl From<&CpuState> for Cpu {
    fn from(state: &CpuState) -> Self {
        OPCODES_TABLE.get_or_init(opcode::init);

        Self {
            program_counter: state.program_counter,
            stack_pointer: state.stack_pointer,
            accumulator: state.accumulator,
            x_register: state.x_register,
            y_register: state.y_register,
            processor_status: state.processor_status,
            lo: state.lo,
            hi: state.hi,
            current_op: state.current_op,
            op_queue: state.op_queue,
            remaining_dma_cycles: state.remaining_dma_cycles,
            current_opcode: get_opcode(state.current_opcode),
            data_bus: state.data_bus,
            ane_constant: state.ane_constant,
            is_halted: state.is_halted,
            irq_state: state.irq_state,
            nmi_state: state.nmi_state,
            dma_state: state.dma_state,
            last_memory_access: None,
            cycle: state.cycle,
        }
    }
}

/// Snapshot of the PPU state at a specific point in time.
///
/// Contains all PPU registers, VRAM, OAM, palette RAM, and internal
/// rendering state. This is part of a [`SaveState`] and is not typically
/// constructed directly by library users.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[allow(clippy::struct_excessive_bools)]
pub struct PpuState {
    /// Total dot cycles elapsed.
    pub cycle_counter: u64,
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
    pub(crate) sprite_zero_in_scanline: bool,
    /// OAM (sprite) memory snapshot (256 bytes).
    pub oam_mem: Vec<u8>,
}

impl From<&Ppu> for PpuState {
    fn from(ppu: &Ppu) -> Self {
        Self {
            cycle_counter: ppu.dot_counter,
            vbl_reset_counter: ppu.vbl_reset_counter,
            status_register: ppu.status_register,
            ctrl_register: ppu.ctrl_register,
            mask_register: ppu.mask_register,
            nmi_requested: ppu.nmi_requested,
            // Only save nametable VRAM (2KB) - addresses 0x2000-0x27FF
            ppu_addr_register: ppu.v_register,
            oam_addr_register: ppu.oam_addr_register,
            write_latch: ppu.write_latch,
            ppu_data_buffer: ppu.data_buffer,
            t_register: ppu.t_register,
            bg_next_tile_id: ppu.bg_next_tile_id,
            bg_next_tile_lsb: ppu.bg_next_tile_lsb,
            vbl_clear_scheduled: ppu.vbl_clear_scheduled,
            prev_vbl: ppu.prev_vbl,
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
            oam_mem: ppu.oam.snapshot_all(),
            bg_next_tile_attribute: ppu.bg_next_tile_attribute,
            fine_x_scroll: ppu.fine_x_scroll,
            even_frame: ppu.even_frame,
            reset_signal: ppu.reset_signal,
            dot: ppu.dot,
            scanline: ppu.scanline,
            sprite_zero_in_scanline: ppu.sprite_zero_in_scanline,
        }
    }
}

impl From<&PpuState> for Ppu {
    fn from(state: &PpuState) -> Self {
        let mut ppu = Self {
            dot_counter: state.cycle_counter,
            ctrl_register: state.ctrl_register,
            mask_register: state.mask_register,
            status_register: state.status_register,
            oam_addr_register: state.oam_addr_register,
            v_register: state.ppu_addr_register,
            data_buffer: state.ppu_data_buffer,
            nmi_requested: state.nmi_requested,
            oam: (&state.oam_mem, true).into(),
            write_latch: state.write_latch,
            t_register: state.t_register,
            bg_next_tile_id: state.bg_next_tile_id,
            bg_next_tile_attribute: state.bg_next_tile_attribute,
            bg_next_tile_lsb: state.bg_next_tile_lsb,
            fine_x_scroll: state.fine_x_scroll,
            even_frame: state.even_frame,
            reset_signal: state.reset_signal,
            pixel_buffer: vec![
                0;
                usize::from(TOTAL_OUTPUT_WIDTH) * usize::from(TOTAL_OUTPUT_HEIGHT)
            ],
            vbl_reset_counter: state.vbl_reset_counter,
            vbl_clear_scheduled: state.vbl_clear_scheduled,
            scanline: state.scanline,
            dot: state.dot,
            prev_vbl: state.prev_vbl,
            address_bus: state.address_bus,
            address_latch: state.address_latch,
            shift_pattern_lo: state.shift_pattern_lo,
            shift_pattern_hi: state.shift_pattern_hi,
            shift_attr_lo: state.shift_attr_lo,
            shift_attr_hi: state.shift_attr_hi,
            shift_in_attr_lo: state.shift_in_attr_lo,
            shift_in_attr_hi: state.shift_in_attr_hi,
            is_soam_clear_active: state.is_soam_clear_active,
            oam_index: state.oam_index,
            soam_index: state.soam_index,
            soam_disable: state.soam_disable,
            oam_increment: state.oam_increment,
            soam_write_counter: state.soam_write_counter, // 1
            oam_fetch: state.oam_fetch,
            current_sprite_tile_id: 0,
            current_sprite_y: 0,
            sprite_fifos: [SpriteFifo::default(); 8],
            sprite_zero_in_scanline: state.sprite_zero_in_scanline,
            log: String::new(),
        };

        ppu.oam.load(state.oam_mem.clone().into_boxed_slice());

        ppu
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApuState {
    frame_counter: FrameCounter,
}

impl From<&Apu> for ApuState {
    fn from(apu: &Apu) -> Self {
        Self {
            frame_counter: apu.frame_counter.clone(),
        }
    }
}

impl From<&ApuState> for Apu {
    fn from(state: &ApuState) -> Self {
        Self {
            frame_counter: state.frame_counter.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PeripheralState {
    StandardController { shift: u8, strobe: bool },
}

impl From<&Peripheral> for PeripheralState {
    fn from(value: &Peripheral) -> Self {
        match value {
            Peripheral::StandardController(s) => PeripheralState::StandardController {
                shift: s.shift,
                strobe: s.strobe,
            },
        }
    }
}

impl From<&PeripheralState> for Peripheral {
    fn from(value: &PeripheralState) -> Self {
        match value {
            PeripheralState::StandardController {
                shift,
                strobe,
            } => Peripheral::StandardController(StandardController::new(*shift, *strobe)),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct BoardState {
    /// Captured CPU state.
    pub cpu: CpuState,
    /// Captured PPU state.
    pub ppu: PpuState,
    pub apu: ApuState,
    pub cpu_ram: Vec<u8>,
    pub nametable_ram: Vec<u8>,
    pub palette_ram: Vec<u8>,
    pub mapper: Mapper,
    pub cpu_open_bus: OpenBus,
    pub ppu_open_bus: OpenBus,
    pub controller1: Option<PeripheralState>,
    pub controller2: Option<PeripheralState>,
    pub joystick_strobe_data: u8,
}

impl From<&Board> for BoardState {
    fn from(board: &Board) -> Self {
        BoardState {
            cpu: CpuState::from(&board.cpu),
            ppu: PpuState::from(&board.ppu),
            apu: ApuState::from(&board.apu),
            cpu_ram: board.cpu_ram.snapshot_all(),
            nametable_ram: board.nametable_ram.snapshot_all(),
            palette_ram: board.palette_ram.snapshot_all(),
            mapper: board.mapper.clone(),
            cpu_open_bus: board.cpu_open_bus,
            ppu_open_bus: board.ppu_open_bus,
            controller1: board.port1.as_ref().map(|p| p.into()),
            controller2: board.port2.as_ref().map(|p| p.into()),
            joystick_strobe_data: board.joystick_strobe_data,
        }
    }
}

impl From<&BoardState> for Board {
    fn from(state: &BoardState) -> Self {
        Board {
            cpu: Cpu::from(&state.cpu),
            ppu: Ppu::from(&state.ppu),
            apu: Apu::from(&state.apu),
            cpu_ram: Memory::from((&state.cpu_ram, true)),
            nametable_ram: Memory::from((&state.nametable_ram, true)),
            palette_ram: PaletteRam::from(&state.palette_ram),
            mapper: state.mapper.clone(),
            cpu_open_bus: state.cpu_open_bus,
            ppu_open_bus: state.ppu_open_bus,
            port1: state.controller1.clone().map(|s| (&s).into()),
            port2: state.controller2.clone().map(|s| (&s).into()),
            joystick_strobe_data: state.joystick_strobe_data,
            irq: false,
        }
    }
}

/// A complete snapshot of the NES emulator state.
///
/// Contains the CPU state, PPU state, loaded ROM metadata, and timing
/// information. Can be serialized with
/// [`ToBytes::to_bytes()`](crate::util::ToBytes::to_bytes) and deserialized
/// with [`try_load_state_from_bytes()`].
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
    pub board: BoardState,
    /// ROM metadata (raw data is skipped in serialization).
    pub rom_file: RomFile,
    /// Save state format version.
    pub version: u16,
    /// Total master clock cycles at the time of capture.
    pub total_cycles: u64,
    pub ppu_cycle_counter: u8,
    /// CPU clock divider counter at the time of capture.
    pub cpu_cycle_counter: u8,
}

assert_impl_all!(SaveState: Sync);

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
#[must_use]
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
