use serde::{Deserialize, Serialize};

use crate::emulation::mapper::nametable_mapping::NametableArrangement;
use crate::emulation::mapper::{
    CpuReadResult, CpuWriteResult, Mapper, MapperLike, PpuReadResult, PpuWriteResult,
};
use crate::emulation::mem::{Memory, OpenBus};
use crate::emulation::ppu::VRAM_SIZE;
use crate::emulation::rom::RomFile;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct MMC1 {
    pub version: u8,
    pub submapper: u8,
    pub prg_ram_size: u16,
    pub prg_ram_battery_backed: bool,
    pub prg_rom_size: u32,
    pub prg_ram: Option<Memory>,
    pub prg_rom: Memory,
    pub chr_rom_size: u32,
    pub chr_rom: Option<Memory>,
    pub nametable_arrangement: NametableArrangement,
    pub prg_ram_bank_offset: u16,
    pub last_shift_write: u128,
    shift: u8,
    shift_count: u8,
    ctrl_reg: u8,
    chr_bank_0: u8,
    chr_bank_1: u8,
    prg_bank: u8,
    prg_rom_bank_mode: u8,
    chr_rom_bank_mode: u8,
}

impl MapperLike for MMC1 {
    #[inline]
    fn write(&mut self, addr: u16, data: u8, cycle: u128) -> CpuWriteResult {
        match addr {
            0x4020..=0xFFFF => {
                if addr >= 6000 && addr <= 0x7FFF {
                    if let Some(prg_ram) = &mut self.prg_ram {
                        let addr = ((addr - 0x6000) + self.prg_ram_bank_offset) % self.prg_ram_size;
                        prg_ram.write(addr as u32, data);
                    }
                }

                if addr >= 0x8000 {
                    if data & 0x80 != 0 {
                        self.shift = 0;
                    } else {
                        if self.last_shift_write != cycle - 1 {
                            self.shift = (self.shift >> 1) | ((data & 1) << 4);
                            self.shift_count += 1;

                            if self.shift_count == 5 {
                                self.copy_shift(addr)
                            }

                            self.shift_count = 0;
                            self.shift = 0;
                        }

                        self.last_shift_write = cycle
                    }
                }

                CpuWriteResult::Handled
            }
            _ => CpuWriteResult::Registered,
        }
    }

    #[inline]
    fn init(&mut self, addr: u16, data: u8) -> CpuWriteResult { self.write(addr, data, 0) }

    #[inline]
    fn read(&mut self, addr: u16, open_bus: &OpenBus) -> CpuReadResult {
        self.read_debug(addr, open_bus)
    }

    #[inline]
    fn read_debug(&self, addr: u16, open_bus: &OpenBus) -> CpuReadResult {
        if (addr >= 0x4000 && addr <= 0x4014) || addr >= 0x4018 {
            let value = match addr {
                0x6000..=0x7FFF => {
                    if let Some(prg_ram) = &self.prg_ram {
                        let addr = ((addr - 0x6000) + self.prg_ram_bank_offset) % self.prg_ram_size;
                        prg_ram.read(addr as u32, open_bus)
                    } else {
                        open_bus.read()
                    }
                }
                0x8000..=0xFFFF => self
                    .prg_rom
                    .read(self.get_prg_rom_address(addr) % self.prg_rom_size, open_bus),
                _ => open_bus.read(),
            };

            return CpuReadResult::Handled(value);
        }

        CpuReadResult::Registered
    }

    #[inline]
    fn ppu_read(&mut self, addr: u16, open_bus: &OpenBus) -> PpuReadResult {
        self.ppu_read_debug(addr, open_bus)
    }

    #[inline]
    fn ppu_read_debug(&self, addr: u16, open_bus: &OpenBus) -> PpuReadResult {
        match addr {
            0..=0x1FFF => {
                if let Some(rom) = &self.chr_rom {
                    PpuReadResult::Handled(rom.read(self.get_chr_rom_address(addr), open_bus))
                } else {
                    PpuReadResult::Registered
                }
            }
            0x2000..=0x3EFF => PpuReadResult::Nametable(
                self.nametable_arrangement.resolve_address(addr - 0x2000) % VRAM_SIZE as u16,
            ),
            _ => PpuReadResult::Registered,
        }
    }

    #[inline]
    fn ppu_write(&mut self, addr: u16, data: u8) -> PpuWriteResult {
        match addr {
            0..=0x1FFF => {
                let address = self.get_chr_rom_address(addr);
                if let Some(rom) = &mut self.chr_rom {
                    rom.write(address, data)
                }
                PpuWriteResult::Handled
            }
            0x2000..=0x3EFF => PpuWriteResult::Nametable(
                self.nametable_arrangement.resolve_address(addr - 0x2000) % VRAM_SIZE as u16,
            ),
            _ => PpuWriteResult::Registered,
        }
    }

    #[inline]
    fn ppu_init(&mut self, addr: u16, data: u8) -> PpuWriteResult { self.ppu_write(addr, data) }
}

const KB_16: u32 = 0x4000;

impl MMC1 {
    #[inline]
    fn get_prg_rom_address(&self, addr: u16) -> u32 {
        // should be between 0x4000 and 0x7FFF
        let addr = (addr - 0x8000) as u32;
        let bank = self.prg_bank & 0b1111;

        // we're in 32kb mode
        if self.prg_rom_bank_mode <= 1 {
            return 0x8000 * ((bank as u32) >> 1) + addr;
        }

        // were in 16kb mode, so the half determines what logic gets used
        let is_in_first_half = addr / KB_16 == 0;

        match self.prg_rom_bank_mode {
            2 => {
                if is_in_first_half {
                    addr
                } else {
                    (bank as u32 * KB_16) + addr
                }
            }
            3 => {
                if is_in_first_half {
                    (bank as u32 * KB_16) + addr
                } else {
                    addr + (15 * KB_16)
                }
            }
            _ => {
                unreachable!()
            }
        }
    }

    #[inline]
    fn get_chr_rom_address(&self, addr: u16) -> u32 {
        let addr = addr as u32;
        let is_8kb_mode = self.ctrl_reg & 0b10000 == 0;

        if is_8kb_mode {
            (self.chr_bank_0 as u32 >> 1) * 0x2000 + addr
        } else {
            let is_in_first_half = addr / 0x1000 == 0;
            let bank = if is_in_first_half {
                self.chr_bank_0
            } else {
                self.chr_bank_1
            };

            (0x2000 * bank as u32) + addr
        }
    }

    fn copy_shift(&mut self, addr: u16) {
        match addr {
            0x8000..=0x9FFF => self.set_ctrl(),
            0xA000..=0xBFFF => {
                self.chr_bank_0 = self.shift;
            }
            0xC000..=0xDFFF => self.chr_bank_1 = self.shift,
            0xE000..=0xFFFF => self.prg_bank = self.shift,
            _ => {}
        }
    }

    fn set_ctrl(&mut self) {
        let nametable = self.shift & 0b11;

        match nametable {
            0 => self.nametable_arrangement = NametableArrangement::SingleScreenLower,
            1 => self.nametable_arrangement = NametableArrangement::SingleScreenUpper,
            2 => self.nametable_arrangement = NametableArrangement::Horizontal,
            3 => self.nametable_arrangement = NametableArrangement::Vertical,
            _ => unreachable!(),
        }

        self.prg_rom_bank_mode = self.shift & 0b1100;
        self.chr_rom_bank_mode = self.shift & 0b10000;
    }
}

impl From<&RomFile> for MMC1 {
    fn from(value: &RomFile) -> Self {
        let prg_ram_size = Mapper::get_likely_correct_ram_size(value);
        let battery_backed = value.is_battery_backed || value.prg_memory.prg_nvram_size > 0;

        MMC1 {
            version: 0,
            submapper: value.submapper_number,
            prg_ram_size: prg_ram_size as u16,
            prg_ram_battery_backed: battery_backed,
            prg_rom_size: value.prg_memory.prg_rom_size,
            prg_rom: value.get_prg_rom(),
            chr_rom: value.get_chr_rom(),
            nametable_arrangement: value.get_nametable_memory(),
            prg_ram: if prg_ram_size > 0 {
                Some(Memory::new(prg_ram_size as usize, true))
            } else {
                None
            },
            chr_rom_size: value.chr_memory.chr_rom_size,
            prg_ram_bank_offset: 0,
            last_shift_write: u128::MAX,
            shift: 0,
            shift_count: 0,
            ctrl_reg: 0,
            chr_bank_0: 0,
            chr_bank_1: 0,
            prg_bank: 0,
            prg_rom_bank_mode: 0,
            chr_rom_bank_mode: 0,
        }
    }
}
