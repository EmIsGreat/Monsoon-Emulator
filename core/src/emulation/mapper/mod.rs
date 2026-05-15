use std::fmt::Debug;
use std::hash::Hash;

use nametable_mapping::NametableArrangement;
use serde::{Deserialize, Serialize};

use crate::emulation::mapper::mmc1::MMC1;
use crate::emulation::mem::{Memory, OpenBus};
use crate::emulation::ppu::VRAM_SIZE;
use crate::emulation::rom::{RomFile, RomMapper};

pub mod mmc1;
pub mod nametable_mapping;

#[enum_delegate::implement(MapperLike)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Mapper {
    NoMapper(NoMapper),
    MMC1(MMC1),
    Nrom(Nrom),
}

impl From<&RomFile> for Mapper {
    fn from(value: &RomFile) -> Self {
        match value.mapper {
            RomMapper::NRom => Mapper::Nrom(Nrom::from(value)),
            RomMapper::MMC1 => Mapper::MMC1(MMC1::from(value)),
            RomMapper::Unknown(_) => Mapper::NoMapper(NoMapper {}),
        }
    }
}

impl Mapper {
    fn get_likely_correct_ram_size(value: &RomFile) -> u32 {
        let battery_backed = value.is_battery_backed || value.prg_memory.prg_nvram_size > 0;

        let mut prg_ram_size = if battery_backed {
            value.prg_memory.prg_nvram_size
        } else {
            value.prg_memory.prg_ram_size
        };

        if prg_ram_size == 0 {
            if value.prg_memory.prg_nvram_size > 0 {
                prg_ram_size = value.prg_memory.prg_nvram_size;
            } else if value.prg_memory.prg_ram_size > 0 {
                prg_ram_size = value.prg_memory.prg_ram_size;
            }
        };

        prg_ram_size
    }
}

#[enum_delegate::register]
pub trait MapperLike {
    fn write(&mut self, addr: u16, data: u8, cycle: u128) -> CpuWriteResult;
    fn init(&mut self, addr: u16, data: u8) -> CpuWriteResult;
    fn read(&mut self, addr: u16, open_bus: &OpenBus) -> CpuReadResult;
    fn read_debug(&self, addr: u16, open_bus: &OpenBus) -> CpuReadResult;
    fn ppu_read(&mut self, addr: u16, open_bus: &OpenBus) -> PpuReadResult;
    fn ppu_read_debug(&self, addr: u16, open_bus: &OpenBus) -> PpuReadResult;
    fn ppu_write(&mut self, addr: u16, data: u8) -> PpuWriteResult;
    fn ppu_init(&mut self, addr: u16, data: u8) -> PpuWriteResult;
}

#[derive(Debug, Clone, Copy)]
pub enum CpuReadResult {
    Handled(u8, bool),
    Registered,
}

#[derive(Debug, Clone, Copy)]
pub enum CpuWriteResult {
    Handled,
    Registered,
}

#[derive(Debug, Clone, Copy)]
pub enum PpuReadResult {
    Handled(u8, bool),
    Nametable(u16),
    Registered,
}

#[derive(Debug, Clone, Copy)]
pub enum PpuWriteResult {
    Handled,
    Nametable(u16),
    Registered,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NoMapper {}

impl MapperLike for NoMapper {
    fn write(&mut self, addr: u16, _: u8, _: u128) -> CpuWriteResult {
        match addr {
            0x4020..=0xFFFF => CpuWriteResult::Handled,
            _ => CpuWriteResult::Registered,
        }
    }

    fn init(&mut self, addr: u16, _: u8) -> CpuWriteResult {
        match addr {
            0x4020..=0xFFFF => CpuWriteResult::Handled,
            _ => CpuWriteResult::Registered,
        }
    }

    fn read(&mut self, addr: u16, open_bus: &OpenBus) -> CpuReadResult {
        if (0x4000..=0x4014).contains(&addr) || addr >= 0x4018 {
            return CpuReadResult::Handled(open_bus.read(), false);
        }

        CpuReadResult::Registered
    }

    fn read_debug(&self, addr: u16, open_bus: &OpenBus) -> CpuReadResult {
        if (0x4000..=0x4014).contains(&addr) || addr >= 0x4018 {
            return CpuReadResult::Handled(open_bus.read(), false);
        }

        CpuReadResult::Registered
    }

    fn ppu_read(&mut self, addr: u16, open_bus: &OpenBus) -> PpuReadResult {
        self.ppu_read_debug(addr, open_bus)
    }

    fn ppu_read_debug(&self, addr: u16, open_bus: &OpenBus) -> PpuReadResult {
        match addr {
            0..=0x1FFF => PpuReadResult::Handled(open_bus.read(), false),
            0x2000..=0x3EFF => PpuReadResult::Nametable((addr - 0x2000) % VRAM_SIZE),
            _ => PpuReadResult::Registered,
        }
    }

    fn ppu_write(&mut self, addr: u16, _: u8) -> PpuWriteResult {
        match addr {
            0..=0x3FFF => PpuWriteResult::Handled,
            _ => PpuWriteResult::Registered,
        }
    }

    fn ppu_init(&mut self, addr: u16, _: u8) -> PpuWriteResult {
        match addr {
            0..=0x3FFF => PpuWriteResult::Handled,
            _ => PpuWriteResult::Registered,
        }
    }
}

impl<'a> From<&'a RomFile> for NoMapper {
    fn from(_: &'a RomFile) -> Self { NoMapper {} }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Nrom {
    pub prg_ram_size: u16,
    pub prg_ram_battery_backed: bool,
    pub prg_rom_size: u16,
    pub prg_ram: Option<Memory>,
    pub prg_rom: Memory,
    pub chr_rom: Option<Memory>,
    pub nametable_arrangement: NametableArrangement,
}

impl MapperLike for Nrom {
    #[inline]
    fn write(&mut self, addr: u16, data: u8, _: u128) -> CpuWriteResult {
        match addr {
            0x4020..=0xFFFF => {
                #[allow(clippy::collapsible_if)]
                if (6000..=0x7FFF).contains(&addr) {
                    if let Some(prg_ram) = &mut self.prg_ram {
                        let addr = (addr - 0x6000) % self.prg_ram_size;
                        prg_ram.write(addr as u32, data);
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

    fn read_debug(&self, addr: u16, open_bus: &OpenBus) -> CpuReadResult {
        if (0x4000..=0x4014).contains(&addr) || addr >= 0x4018 {
            let (val, update) = match addr {
                0x6000..=0x7FFF => {
                    if let Some(prg_ram) = &self.prg_ram {
                        let addr = (addr - 0x6000) % self.prg_ram_size;
                        (prg_ram.read(addr as u32, open_bus), true)
                    } else {
                        (open_bus.read(), false)
                    }
                }
                0x8000..=0xFFFF => (
                    self.prg_rom
                        .read(((addr - 0x8000) % self.prg_rom_size) as u32, open_bus),
                    true,
                ),
                _ => (open_bus.read(), false),
            };

            return CpuReadResult::Handled(val, update);
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
                    PpuReadResult::Handled(rom.read(addr as u32, open_bus), true)
                } else {
                    PpuReadResult::Handled(open_bus.read(), false)
                }
            }
            0x2000..=0x3EFF => {
                PpuReadResult::Nametable(self.nametable_arrangement.resolve_address(addr))
            }
            _ => PpuReadResult::Registered,
        }
    }

    #[inline]
    fn ppu_write(&mut self, addr: u16, data: u8) -> PpuWriteResult {
        match addr {
            0..=0x1FFF => {
                if let Some(rom) = &mut self.chr_rom {
                    rom.write(addr as u32, data)
                }
                PpuWriteResult::Handled
            }
            0x2000..=0x3EFF => {
                PpuWriteResult::Nametable(self.nametable_arrangement.resolve_address(addr))
            }
            _ => PpuWriteResult::Registered,
        }
    }

    #[inline]
    fn ppu_init(&mut self, addr: u16, data: u8) -> PpuWriteResult { self.ppu_write(addr, data) }
}

impl From<&RomFile> for Nrom {
    fn from(rom: &RomFile) -> Self {
        let prg_ram_size = Mapper::get_likely_correct_ram_size(rom);
        let battery_backed = rom.is_battery_backed || rom.prg_memory.prg_nvram_size > 0;

        Nrom {
            prg_ram_battery_backed: battery_backed,
            prg_ram_size: prg_ram_size as u16,
            prg_rom_size: rom.prg_memory.prg_rom_size as u16,
            prg_rom: rom.get_prg_rom(),
            chr_rom: rom.get_chr_rom(),
            nametable_arrangement: rom.get_nametable_arrangement(),
            prg_ram: if prg_ram_size > 0 {
                Some(Memory::new(prg_ram_size as usize, true))
            } else {
                None
            },
        }
    }
}
