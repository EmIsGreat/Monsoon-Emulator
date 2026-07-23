use serde::{Deserialize, Serialize};

use crate::emulation::mem::{Memory, OpenBus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteRam {
    zero_bits: [u8; 4],
    palettes: Memory,
}

impl PaletteRam {}

impl Default for PaletteRam {
    fn default() -> Self {
        Self {
            zero_bits: [0; 4],
            palettes: Memory::new(0x20, true),
        }
    }
}

impl PaletteRam {
    #[inline]
    pub fn read(&self, addr: u16, open_bus: &OpenBus) -> u8 {
        let val = match addr {
            0x0 | 0x4 | 0x8 | 0xC | 0x10 | 0x14 | 0x18 | 0x1C => {
                self.zero_bits[(addr % 0x10) as usize / 4usize]
            }
            _ => self.palettes.read(u32::from(addr), open_bus),
        };

        (val & 0b0011_1111) | (open_bus.read() & 0b1100_0000)
    }

    #[inline]
    pub fn snapshot(&self, addr: u16, open_bus: &OpenBus) -> u8 { self.read(addr, open_bus) }

    #[inline]
    pub fn write(&mut self, addr: u16, data: u8) {
        let data = data & 0b0011_1111;
        match addr {
            0x0 | 0x4 | 0x8 | 0xC | 0x10 | 0x14 | 0x18 | 0x1C => {
                self.zero_bits[(addr % 0x10) as usize / 4usize] = data;
            }
            _ => self.palettes.write(u32::from(addr), data),
        }
    }

    #[inline]
    pub fn init(&mut self, addr: u16, data: u8) { self.write(addr, data) }

    #[inline]
    pub fn snapshot_all(&self) -> Vec<u8> {
        let range = 0..=0x20u16;
        let mut vec = Vec::with_capacity(range.len());
        range.for_each(|addr| vec.push(self.read(addr, &OpenBus::new(0))));
        vec
    }
}

impl From<&Vec<u8>> for PaletteRam {
    #[allow(clippy::cast_possible_truncation)]
    fn from(value: &Vec<u8>) -> Self {
        let mut palette_ram = PaletteRam::default();
        value
            .iter()
            .enumerate()
            .for_each(|(i, b)| palette_ram.write(i as u16, *b));
        palette_ram
    }
}
