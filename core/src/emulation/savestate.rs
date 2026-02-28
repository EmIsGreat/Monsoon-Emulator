use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::emulation::cpu::{Cpu, INTERNAL_RAM_SIZE, MicroOp};
use crate::emulation::mem::OpenBus;
use crate::emulation::ppu::{Ppu, VRAM_SIZE};
use crate::emulation::rom::RomFile;

pub const MAGIC: &[u8; 5] = b"ESSV1"; // NES SaveState
pub const BINARY_FORMAT_VERSION: u8 = 0;
pub const JSON_FORMAT_VERSION: u8 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CpuState {
    pub program_counter: u16,
    pub stack_pointer: u8,
    pub accumulator: u8,
    pub x_register: u8,
    pub y_register: u8,
    pub processor_status: u8,
    /// Internal RAM only (2KB, addresses 0x0000-0x07FF)
    pub internal_ram: Vec<u8>,
    /// PRG RAM if present (up to 8KB, addresses 0x6000-0x7FFF)
    pub prg_ram: Vec<u8>,
    /// Full memory dump for tracing/debug (not serialized to reduce size)
    #[serde(skip)]
    pub memory: Vec<u8>,
    pub lo: u8,
    pub hi: u8,
    pub current_op: MicroOp,
    pub op_queue: VecDeque<MicroOp>,
    pub current_opcode: Option<u8>,
    pub temp: u8,
    pub ane_constant: u8,
    pub is_halted: bool,
    pub read_cycle: bool,
    pub irq_detected: bool,
    pub irq_pending: bool,
    pub irq_provider: bool,
    pub is_in_irq: bool,
    pub current_irq_vec: u16,
    pub locked_irq_vec: u16,
    pub dma_page: u8,
    pub dma_read: bool,
    pub dma_temp: u8,
    pub dma_triggered: bool,
    pub nmi_detected: bool,
    pub nmi_pending: bool,
    pub prev_nmi: bool,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PpuState {
    pub cycle_counter: u128,
    pub vbl_reset_counter: u8,
    pub status_register: u8,
    pub ctrl_register: u8,
    pub mask_register: u8,
    pub nmi_requested: bool,
    /// Nametable VRAM only (2KB, addresses 0x2000-0x27FF mirrored)
    pub nametable_ram: Vec<u8>,
    pub ppu_addr_register: u16,
    pub oam_addr_register: u8,
    pub write_latch: bool,
    pub ppu_data_buffer: u8,
    pub t_register: u16,
    pub bg_next_tile_attribute: u8,
    pub fine_x_scroll: u8,
    pub even_frame: bool,
    pub reset_signal: bool,
    pub dot: u16,
    pub scanline: u16,
    pub bg_next_tile_id: u8,
    pub bg_next_tile_lsb: u8,
    pub vbl_clear_scheduled: Option<u8>,
    pub prev_vbl: u8,
    pub open_bus: OpenBus,
    pub address_bus: u16,
    pub address_latch: u8,
    pub shift_pattern_lo: u16,
    pub shift_pattern_hi: u16,
    pub shift_attr_lo: u8,
    pub shift_attr_hi: u8,
    pub shift_in_attr_lo: bool,
    pub shift_in_attr_hi: bool,
    pub is_soam_clear_active: bool,
    pub oam_index: u8,
    pub soam_index: u8,
    pub soam_disable: bool,
    pub oam_increment: u8,
    pub soam_write_counter: u8,
    pub oam_fetch: u8,
    pub oam_mem: Vec<u8>,
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

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SaveState {
    pub cpu: CpuState,
    pub ppu: PpuState,
    pub rom_file: RomFile,
    pub version: u16,
    pub total_cycles: u128,
    pub ppu_cycle_counter: u8,
    pub cpu_cycle_counter: u8,
}

/// Try to load a savestate from raw bytes, returning None on error
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
