use std::fmt::Debug;
use std::hash::Hash;

use enum_dispatch::enum_dispatch;
use nametable_mapping::NametableArrangement;
use serde::{Deserialize, Serialize};

use crate::emulation::mem::{MemoryDevice, OpenBus, Ram, Rom};
use crate::emulation::ppu::{PALETTE_RAM_SIZE, VRAM_SIZE};
use crate::emulation::rom::{RomFile, RomMapper};

pub mod nametable_mapping;

#[enum_dispatch(MapperLike)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Mapper {
    NoMapper,
    Nrom,
}

impl From<&RomFile> for Mapper {
    fn from(value: &RomFile) -> Self {
        match value.mapper {
            RomMapper::NRom => {
                let battery_backed = value.is_battery_backed || value.prg_memory.prg_nvram_size > 0;
                let mut prg_ram_size = if battery_backed {
                    value.prg_memory.prg_nvram_size
                } else {
                    value.prg_memory.prg_ram_size
                };

                if prg_ram_size == 0 {
                    if value.prg_memory.prg_nvram_size > 0 {
                        prg_ram_size = value.prg_memory.prg_ram_size;
                    } else if value.prg_memory.prg_ram_size > 0 {
                        prg_ram_size = value.prg_memory.prg_ram_size;
                    }
                }

                Mapper::Nrom(Nrom {
                    prg_ram_battery_backed: battery_backed,
                    prg_ram_size: prg_ram_size as u16,
                    prg_rom_size: value.prg_memory.prg_rom_size as u16,
                    prg_rom: value.get_prg_rom(),
                    chr_rom: value.get_chr_rom(),
                    nametable_arrangement: value.get_nametable_memory(),
                    prg_ram: if prg_ram_size > 0 {
                        Some(Ram::new(prg_ram_size as usize))
                    } else {
                        None
                    },
                })
            }
            RomMapper::Unknown(_) => Mapper::NoMapper(NoMapper {}),
        }
    }
}

#[enum_dispatch]
pub trait MapperLike: Debug + Eq + PartialEq + Hash + Clone {
    fn write(&mut self, addr: u16, data: u8) -> CpuWriteResult;
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
    Handled(u8),
    Registered,
}

#[derive(Debug, Clone, Copy)]
pub enum CpuWriteResult {
    Handled,
    Registered,
}

#[derive(Debug, Clone, Copy)]
pub enum PpuReadResult {
    Handled(u8),
    Nametable(u16),
    Palette(u16),
    Registered,
}

#[derive(Debug, Clone, Copy)]
pub enum PpuWriteResult {
    Handled,
    Nametable(u16),
    Palette(u16),
    Registered,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NoMapper {}

impl MapperLike for NoMapper {
    fn write(&mut self, addr: u16, _: u8) -> CpuWriteResult {
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
        if (addr >= 0x4000 && addr <= 0x4014) || addr >= 0x4018 {
            return CpuReadResult::Handled(open_bus.read());
        }

        CpuReadResult::Registered
    }

    fn read_debug(&self, addr: u16, open_bus: &OpenBus) -> CpuReadResult {
        if (addr >= 0x4000 && addr <= 0x4014) || addr >= 0x4018 {
            return CpuReadResult::Handled(open_bus.read());
        }

        CpuReadResult::Registered
    }

    fn ppu_read(&mut self, addr: u16, open_bus: &OpenBus) -> PpuReadResult {
        self.ppu_read_debug(addr, open_bus)
    }

    fn ppu_read_debug(&self, addr: u16, open_bus: &OpenBus) -> PpuReadResult {
        match addr {
            0..=0x1FFF => PpuReadResult::Handled(open_bus.read()),
            0x2000..=0x3EFF => PpuReadResult::Nametable((addr - 0x2000) % (VRAM_SIZE as u16)),
            0x3F00..=0x3FFF => PpuReadResult::Palette((addr - 0x3F00) % PALETTE_RAM_SIZE),
            _ => PpuReadResult::Handled(open_bus.read()),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Nrom {
    pub prg_ram_size: u16,
    pub prg_ram_battery_backed: bool,
    pub prg_rom_size: u16,
    pub prg_ram: Option<Ram>,
    pub prg_rom: Rom,
    pub chr_rom: Option<Rom>,
    pub nametable_arrangement: NametableArrangement,
}

impl MapperLike for Nrom {
    #[inline]
    fn write(&mut self, addr: u16, data: u8) -> CpuWriteResult {
        match addr {
            0x4020..=0xFFFF => {
                if addr >= 6000 && addr <= 0x7FFF {
                    if let Some(prg_ram) = &mut self.prg_ram {
                        let addr = (addr - 0x6000) % self.prg_ram_size;
                        prg_ram.write(addr, data);
                    }
                }

                CpuWriteResult::Handled
            }
            _ => CpuWriteResult::Registered,
        }
    }

    #[inline]
    fn init(&mut self, addr: u16, data: u8) -> CpuWriteResult { self.write(addr, data) }

    #[inline]
    fn read(&mut self, addr: u16, open_bus: &OpenBus) -> CpuReadResult {
        self.read_debug(addr, open_bus)
    }

    fn read_debug(&self, addr: u16, open_bus: &OpenBus) -> CpuReadResult {
        if (addr >= 0x4000 && addr <= 0x4014) || addr >= 0x4018 {
            let value = match addr {
                0x6000..=0x7FFF => {
                    if let Some(prg_ram) = &self.prg_ram {
                        let addr = (addr - 0x6000) % self.prg_ram_size;
                        prg_ram.read(addr, &open_bus)
                    } else {
                        open_bus.read()
                    }
                }
                0x8000..=0xFFFF => self
                    .prg_rom
                    .read((addr - 0x8000) % self.prg_rom_size, &open_bus),
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
                    PpuReadResult::Handled(rom.read(addr, open_bus))
                } else {
                    PpuReadResult::Handled(open_bus.read())
                }
            },
            0x2000..=0x3EFF => PpuReadResult::Nametable(
                self.nametable_arrangement.resolve_address(addr - 0x2000) % VRAM_SIZE as u16,
            ),
            0x3F00..=0x3FFF => PpuReadResult::Palette((addr - 0x3F00) % PALETTE_RAM_SIZE),
            _ => PpuReadResult::Registered,
        }
    }

    #[inline]
    fn ppu_write(&mut self, addr: u16, data: u8) -> PpuWriteResult {
        match addr {
            0..=0x1FFF => {
                if let Some(rom) = &mut self.chr_rom {
                    rom.write(addr, data)
                }
                PpuWriteResult::Handled
            }
            0x2000..=0x3EFF => PpuWriteResult::Nametable(
                self.nametable_arrangement.resolve_address(addr - 0x2000) % VRAM_SIZE as u16,
            ),
            0x3F00..=0x3FFF => PpuWriteResult::Palette((addr - 0x3F00) % PALETTE_RAM_SIZE),
            _ => PpuWriteResult::Registered,
        }
    }

    #[inline]
    fn ppu_init(&mut self, addr: u16, data: u8) -> PpuWriteResult {
        self.ppu_write(addr, data)
    }
}
