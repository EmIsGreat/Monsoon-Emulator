use crate::emulation::mem::{MemoryDevice, Ram};

#[derive(Debug, Clone)]
pub struct PaletteRam {
    zero_bits: [u8; 4],
    palettes: Ram,
}

impl Default for PaletteRam {
    fn default() -> Self {
        Self {
            zero_bits: [0; 4],
            palettes: Ram::new(0x20),
        }
    }
}

impl MemoryDevice for PaletteRam {
    #[inline]
    fn read(&self, addr: u16, open_bus: u8) -> u8 {
        match addr {
            0x0 | 0x4 | 0x8 | 0xC | 0x10 | 0x14 | 0x18 | 0x1C => {
                self.zero_bits[(addr % 0x10) as usize / 4usize]
            }
            _ => self.palettes.read(addr, open_bus),
        }
    }

    #[inline]
    fn write(&mut self, addr: u16, data: u8) {
        let data = data & 0b00111111;
        match addr {
            0x0 | 0x4 | 0x8 | 0xC | 0x10 | 0x14 | 0x18 | 0x1C => {
                self.zero_bits[(addr % 0x10) as usize / 4usize] = data
            }
            _ => self.palettes.write(addr, data),
        }
    }

    #[inline]
    fn init(&mut self, addr: u16, data: u8) { self.write(addr, data) }

    #[inline]
    fn load(&mut self, data: Box<[u8]>) {
        for (i, value) in data.iter().enumerate() {
            match i {
                0x0 | 0x4 | 0x8 | 0xC => self.zero_bits[i / 4] = *value,
                _ => self.palettes.write(i as u16, *value),
            }
        }
    }

    #[inline]
    fn snapshot_all(&self) -> Vec<u8> {
        let mut out = self.palettes.snapshot_all();
        out.extend(self.zero_bits);
        out
    }
}
