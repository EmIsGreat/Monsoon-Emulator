use crate::emulation::mem::{MemoryDevice, Ram};
use crate::emulation::ppu::{NAMETABLE_SIZE, VRAM_SIZE};

#[derive(Debug, Clone, Copy)]
pub enum NametableArrangement {
    Vertical,
    Horizontal,
    SingleScreenLower,
    SingleScreenUpper,
    FourScreen,
}

#[derive(Debug, Clone)]
pub struct NametableMemory {
    vram: Ram,
    mirroring: NametableArrangement,
}

impl NametableMemory {
    pub fn new(mirroring: NametableArrangement) -> Self {
        Self {
            vram: Ram::new(VRAM_SIZE), // 2KB of VRAM
            mirroring,
        }
    }

    /// Translate PPU address (0x2000–0x3EFF) to physical VRAM offset
    #[inline(always)]
    fn mirror_addr(&self, addr: u16) -> u16 {
        let table = addr / NAMETABLE_SIZE;
        let offset = addr % NAMETABLE_SIZE;

        match self.mirroring {
            NametableArrangement::Vertical => match table {
                0 | 2 => offset,
                1 | 3 => 0x400 + offset,
                _ => unreachable!(),
            },
            NametableArrangement::Horizontal => match table {
                0 | 1 => offset,
                2 | 3 => 0x400 + offset,
                _ => unreachable!(),
            },
            NametableArrangement::SingleScreenLower => offset,
            NametableArrangement::SingleScreenUpper => 0x400 + offset,
            NametableArrangement::FourScreen => addr,
        }
    }
}

impl MemoryDevice for NametableMemory {
    #[inline(always)]
    fn read(&self, addr: u16, open_bus: u8) -> u8 {
        self.vram.read(self.mirror_addr(addr), open_bus)
    }

    #[inline(always)]
    fn write(&mut self, addr: u16, data: u8) { self.vram.write(self.mirror_addr(addr), data) }

    #[inline(always)]
    fn init(&mut self, addr: u16, data: u8) { self.vram.init(self.mirror_addr(addr), data) }

    #[inline]
    fn load(&mut self, data: Box<[u8]>) { self.vram.load(data) }

    #[inline]
    fn snapshot_all(&self) -> Vec<u8> { self.vram.snapshot_all() }
}
