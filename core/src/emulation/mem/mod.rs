use std::fmt::{Debug, Formatter};

use serde::{Deserialize, Serialize};

use crate::emulation::mem::apu_registers::ApuRegisters;
use crate::emulation::mem::mirror_memory::MirrorMemory;
use crate::emulation::mem::nametable_memory::NametableMemory;
use crate::emulation::mem::palette_ram::PaletteRam;
use crate::emulation::mem::ppu_registers::PpuRegisters;

pub mod apu_registers;
pub mod memory_map;
pub mod mirror_memory;
pub mod nametable_memory;
pub mod palette_ram;
pub mod ppu_registers;

#[derive(Clone)]
pub enum Memory {
    Ram(Ram),
    Rom(Rom),
    MirrorMemory(MirrorMemory),
    PaletteRam(PaletteRam),
    PpuRegisters(PpuRegisters),
    ApuRegisters(ApuRegisters),
    NametableMemory(NametableMemory),
}

impl Debug for Memory {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Memory::Ram(ram) => ram.fmt(f),
            Memory::Rom(rom) => rom.fmt(f),
            Memory::MirrorMemory(mirror_memory) => mirror_memory.fmt(f),
            Memory::PaletteRam(palette_ram) => palette_ram.fmt(f),
            Memory::PpuRegisters(ppu_registers) => ppu_registers.fmt(f),
            Memory::ApuRegisters(apu_registers) => apu_registers.fmt(f),
            Memory::NametableMemory(nametable_memory) => nametable_memory.fmt(f),
        }
    }
}

impl MemoryDevice for Memory {
    #[inline(always)]
    fn read(&self, addr: u16, open_bus: u8) -> u8 {
        match self {
            Memory::Ram(ram) => ram.read(addr, open_bus),
            Memory::Rom(rom) => rom.read(addr, open_bus),
            Memory::MirrorMemory(mirror_memory) => mirror_memory.read(addr, open_bus),
            Memory::PaletteRam(palette_ram) => palette_ram.read(addr, open_bus),
            Memory::PpuRegisters(ppu_registers) => ppu_registers.read(addr, open_bus),
            Memory::ApuRegisters(apu_registers) => apu_registers.read(addr, open_bus),
            Memory::NametableMemory(nametable_memory) => nametable_memory.read(addr, open_bus),
        }
    }

    #[inline(always)]
    fn write(&mut self, addr: u16, data: u8) {
        match self {
            Memory::Ram(ram) => ram.write(addr, data),
            Memory::Rom(rom) => rom.write(addr, data),
            Memory::MirrorMemory(mirror_memory) => mirror_memory.write(addr, data),
            Memory::PaletteRam(palette_ram) => palette_ram.write(addr, data),
            Memory::PpuRegisters(ppu_registers) => ppu_registers.write(addr, data),
            Memory::ApuRegisters(apu_registers) => apu_registers.write(addr, data),
            Memory::NametableMemory(nametable_memory) => nametable_memory.write(addr, data),
        }
    }

    #[inline(always)]
    fn init(&mut self, addr: u16, data: u8) {
        match self {
            Memory::Ram(ram) => ram.init(addr, data),
            Memory::Rom(rom) => rom.init(addr, data),
            Memory::MirrorMemory(mirror_memory) => mirror_memory.init(addr, data),
            Memory::PaletteRam(palette_ram) => palette_ram.init(addr, data),
            Memory::PpuRegisters(ppu_registers) => ppu_registers.init(addr, data),
            Memory::ApuRegisters(apu_registers) => apu_registers.init(addr, data),
            Memory::NametableMemory(nametable_memory) => nametable_memory.init(addr, data),
        }
    }

    #[inline(always)]
    fn load(&mut self, data: Box<[u8]>) {
        match self {
            Memory::Ram(ram) => ram.load(data),
            Memory::Rom(rom) => rom.load(data),
            Memory::MirrorMemory(mirror_memory) => mirror_memory.load(data),
            Memory::PaletteRam(palette_ram) => palette_ram.load(data),
            Memory::PpuRegisters(ppu_registers) => ppu_registers.load(data),
            Memory::ApuRegisters(apu_registers) => apu_registers.load(data),
            Memory::NametableMemory(nametable_memory) => nametable_memory.load(data),
        }
    }

    #[inline(always)]
    fn is_internal(&self) -> bool {
        match self {
            Memory::Ram(ram) => ram.is_internal(),
            Memory::Rom(rom) => rom.is_internal(),
            Memory::MirrorMemory(mirror_memory) => mirror_memory.is_internal(),
            Memory::PaletteRam(palette_ram) => palette_ram.is_internal(),
            Memory::PpuRegisters(ppu_registers) => ppu_registers.is_internal(),
            Memory::ApuRegisters(apu_registers) => apu_registers.is_internal(),
            Memory::NametableMemory(nametable_memory) => nametable_memory.is_internal(),
        }
    }

    #[inline(always)]
    fn snapshot(&self, addr: u16, open_bus: u8) -> u8 {
        match self {
            Memory::Ram(ram) => ram.snapshot(addr, open_bus),
            Memory::Rom(rom) => rom.snapshot(addr, open_bus),
            Memory::MirrorMemory(mirror_memory) => mirror_memory.snapshot(addr, open_bus),
            Memory::PaletteRam(palette_ram) => palette_ram.snapshot(addr, open_bus),
            Memory::PpuRegisters(ppu_registers) => ppu_registers.snapshot(addr, open_bus),
            Memory::ApuRegisters(apu_registers) => apu_registers.snapshot(addr, open_bus),
            Memory::NametableMemory(nametable_memory) => nametable_memory.snapshot(addr, open_bus),
        }
    }

    fn snapshot_all(&self) -> Vec<u8> {
        match self {
            Memory::Ram(ram) => ram.snapshot_all(),
            Memory::Rom(rom) => rom.snapshot_all(),
            Memory::MirrorMemory(mirror_memory) => mirror_memory.snapshot_all(),
            Memory::PaletteRam(palette_ram) => palette_ram.snapshot_all(),
            Memory::PpuRegisters(ppu_registers) => ppu_registers.snapshot_all(),
            Memory::ApuRegisters(apu_registers) => apu_registers.snapshot_all(),
            Memory::NametableMemory(nametable_memory) => nametable_memory.snapshot_all(),
        }
    }
}

pub trait MemoryDevice: Debug {
    fn read(&self, addr: u16, open_bus: u8) -> u8;
    fn write(&mut self, addr: u16, data: u8);
    fn init(&mut self, addr: u16, data: u8);
    fn load(&mut self, data: Box<[u8]>);

    fn is_internal(&self) -> bool { false }
    fn snapshot(&self, addr: u16, open_bus: u8) -> u8 { self.read(addr, open_bus) }
    fn snapshot_all(&self) -> Vec<u8>;
}

#[derive(Debug, Clone)]
pub struct Ram {
    memory: Box<[u8]>,
}

impl Ram {
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "RAM size must be greater than zero");

        Self {
            memory: vec![0; size].into_boxed_slice(),
        }
    }
}

impl MemoryDevice for Ram {
    #[inline(always)]
    fn read(&self, addr: u16, _: u8) -> u8 { self.memory[addr as usize % self.memory.len()] }

    #[inline(always)]
    fn write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize % self.memory.len()] = data;
    }

    #[inline(always)]
    fn init(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize % self.memory.len()] = data;
    }

    fn load(&mut self, data: Box<[u8]>) { self.memory = data }

    fn snapshot_all(&self) -> Vec<u8> { self.memory.to_vec() }
}

#[derive(Debug, Clone)]
pub struct Rom {
    memory: Box<[u8]>,
}

impl Rom {
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "ROM size must be greater than zero");

        Self {
            memory: vec![0; size].into_boxed_slice(),
        }
    }
}

impl MemoryDevice for Rom {
    #[inline(always)]
    fn read(&self, addr: u16, _: u8) -> u8 { self.memory[addr as usize % self.memory.len()] }

    #[inline(always)]
    fn write(&mut self, _: u16, _: u8) {}

    #[inline(always)]
    fn init(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize % self.memory.len()] = data;
    }

    fn load(&mut self, data: Box<[u8]>) { self.memory = data }

    fn snapshot_all(&self) -> Vec<u8> { self.memory.to_vec() }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct OpenBus {
    bits: [BitState; 8],
    decay_time: u32,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BitState {
    set: bool,
    decay_timer: u32,
}

impl OpenBus {
    pub fn new(decay_time: u32) -> Self {
        Self {
            bits: [BitState {
                set: false,
                decay_timer: 0,
            }; 8],
            decay_time,
        }
    }

    #[inline(always)]
    pub fn set_masked(&mut self, value: u8, mask: u8) {
        for bit in 0..8 {
            let bit_mask = 1 << bit;
            if mask & bit_mask != 0 {
                let val = (value & bit_mask) != 0;
                self.bits[bit].set = val;
                self.bits[bit].decay_timer = 0;
            }
        }
    }

    #[inline(always)]
    pub fn tick(&mut self, times: u8) {
        let times_u32 = times as u32;
        for bit in &mut self.bits {
            bit.decay_timer += times_u32;
            if bit.decay_timer > self.decay_time {
                bit.set = false;
                bit.decay_timer = 0
            }
        }
    }

    #[inline(always)]
    pub fn read(&self) -> u8 {
        self.bits
            .iter()
            .enumerate()
            .fold(0u8, |acc, (i, b)| acc | ((b.set as u8) << i))
    }
}
