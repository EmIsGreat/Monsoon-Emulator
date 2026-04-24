use std::ops::{Deref, DerefMut, RangeInclusive};

use crate::emulation::board::CpuBus;
use crate::emulation::cpu::Cpu as CoreCpu;
use crate::emulation::mem::{Memory, OpenBus};
use crate::emulation::nes::ExecutionFinished;

struct TestBus {
    memory: [u8; 0x10000],
    ppu_open_bus: OpenBus,
    nmi: bool,
    irq: bool,
    rom_mapped: bool,
}

impl Default for TestBus {
    fn default() -> Self {
        Self {
            memory: [0; 0x10000],
            ppu_open_bus: OpenBus::new(0),
            nmi: false,
            irq: false,
            rom_mapped: false,
        }
    }
}

impl CpuBus for TestBus {
    fn read(&mut self, addr: u16) -> u8 { self.read_debug(addr) }

    fn read_debug(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.memory[(addr as usize) % 0x800],
            _ => self.memory[addr as usize],
        }
    }

    fn get_range(&self, addr: RangeInclusive<u16>) -> Vec<u8> {
        addr.into_iter().map(|a| self.read_debug(a)).collect()
    }

    fn write(&mut self, addr: u16, data: u8, _: u128) {
        if self.rom_mapped && addr >= 0x8000 {
            return;
        }

        match addr {
            0x0000..=0x1FFF => self.memory[(addr as usize) % 0x800] = data,
            _ => self.memory[addr as usize] = data,
        }
    }

    fn init(&mut self, addr: u16, data: u8) { self.write(addr, data, 0); }

    fn get_ppu_open_bus(&mut self) -> &mut OpenBus { &mut self.ppu_open_bus }

    fn poll_nmi(&mut self) -> bool { self.nmi }

    fn poll_irq(&mut self) -> bool { self.irq }

    fn set_irq(&mut self, val: bool) { self.irq = val; }
}

pub(crate) struct Cpu {
    cpu: CoreCpu,
    bus: TestBus,
}

impl Cpu {
    pub(crate) fn new() -> Self {
        Self {
            cpu: CoreCpu::new(),
            bus: TestBus::default(),
        }
    }

    pub(crate) fn test_instance() -> Self {
        Self {
            cpu: CoreCpu::test_instance(),
            bus: TestBus::default(),
        }
    }

    #[inline]
    fn with_bus<T>(&mut self, f: impl FnOnce(&mut CoreCpu, &mut TestBus) -> T) -> T {
        f(&mut self.cpu, &mut self.bus)
    }

    #[inline]
    pub(crate) fn mem_read(&mut self, addr: u16) -> u8 {
        self.with_bus(|cpu, bus| cpu.mem_read(addr, bus))
    }

    #[inline]
    pub(crate) fn mem_write(&mut self, addr: u16, data: u8) {
        self.with_bus(|cpu, bus| cpu.mem_write(addr, data, bus))
    }

    #[inline]
    pub(crate) fn mem_read_u16(&mut self, addr: u16) -> u16 {
        self.with_bus(|cpu, bus| cpu.mem_read_u16(addr, bus))
    }

    #[inline]
    pub(crate) fn mem_write_u16(&mut self, addr: u16, data: u16) {
        self.with_bus(|cpu, bus| cpu.mem_write_u16(addr, data, bus))
    }

    #[inline]
    pub(crate) fn stack_push(&mut self, data: Option<u8>) {
        self.with_bus(|cpu, bus| cpu.stack_push(data, bus))
    }

    #[inline]
    #[allow(unused_results)]
    pub(crate) fn step(&mut self) -> Result<ExecutionFinished, String> {
        self.with_bus(|cpu, bus| cpu.step(bus))
    }

    #[cfg(test)]
    pub(crate) fn attach_test_rom(&mut self, prg_rom: Memory) {
        let prg_data = prg_rom.snapshot_all();
        if prg_data.is_empty() {
            return;
        }

        self.bus.rom_mapped = true;
        for addr in 0x8000u16..=0xFFFF {
            let offset = ((addr as usize) - 0x8000) % prg_data.len();
            self.bus.memory[addr as usize] = prg_data[offset];
        }
    }
}

impl Deref for Cpu {
    type Target = CoreCpu;

    fn deref(&self) -> &Self::Target { &self.cpu }
}

impl DerefMut for Cpu {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.cpu }
}

#[cfg(test)]
mod brk_vector;
mod instr_misc;
#[cfg(test)]
mod instr_test_v5;
#[cfg(test)]
mod mem;
#[cfg(test)]
mod nmi;
#[cfg(test)]
mod opcodes;
#[cfg(test)]
mod reset;
