use std::cell::RefCell;
use std::fmt::Debug;
use std::hash::Hash;
use std::rc::Rc;
use nametable_mapping::NametableArrangement;
use serde::{Deserialize, Serialize};

use crate::emulation::mapper::mmc1::MMC1;
use crate::emulation::mem::{Memory, OpenBus};
use crate::emulation::ppu::VRAM_SIZE;
use crate::emulation::ppu_util::{
    MapperRegisterTables, RegisterEntry, RegisterFormat, RegisterMap, RegisterValue,
};
use crate::emulation::rom::{RomFile, RomMapper};

pub mod mmc1;
pub mod nametable_mapping;

#[enum_delegate::implement(MapperLike)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Mapper {
    NoMapper(NoMapper),
    NROM(NROM),
    MMC1(MMC1),
}

impl From<&RomFile> for Mapper {
    fn from(value: &RomFile) -> Self {
        match value.mapper {
            RomMapper::NRom => Mapper::NROM(NROM::from(value)),
            RomMapper::MMC1 | RomMapper::MMC1A => Mapper::MMC1(MMC1::from(value)),
            _ => Mapper::NoMapper(NoMapper {}),
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
        }

        prg_ram_size
    }
}

#[enum_delegate::register]
pub trait MapperLike {
    fn write(&mut self, addr: u16, data: u8, cycle: u64) -> CpuWriteResult;
    fn init(&mut self, addr: u16, data: u8) -> CpuWriteResult;
    fn read(&mut self, addr: u16, open_bus: &OpenBus) -> CpuReadResult;
    fn read_debug(&self, addr: u16, open_bus: &OpenBus) -> CpuReadResult;
    fn ppu_read(&mut self, addr: u16, open_bus: &OpenBus) -> PpuReadResult;
    fn ppu_read_debug(&self, addr: u16, open_bus: &OpenBus) -> PpuReadResult;
    fn ppu_write(&mut self, addr: u16, data: u8) -> PpuWriteResult;
    fn ppu_init(&mut self, addr: u16, data: u8) -> PpuWriteResult;
    fn get_registers_debug(&self) -> MapperRegisterTables;
    fn poll_irq(&self) -> bool;
    fn build_ppu_map(&mut self);
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
pub struct NoMapper;

impl MapperLike for NoMapper {
    fn write(&mut self, addr: u16, _: u8, _: u64) -> CpuWriteResult {
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

    fn get_registers_debug(&self) -> MapperRegisterTables {
        let mut state = RegisterMap::new();
        state.insert(
            "name".to_string(),
            RegisterEntry::new(
                RegisterValue::Text("NoMapper".to_string()),
                RegisterFormat::Text,
            ),
        );
        let mut tables = MapperRegisterTables::new();
        tables.insert("General".to_string(), state);
        tables
    }

    fn poll_irq(&self) -> bool { false }

    fn build_ppu_map(&mut self) {}
}

impl<'a> From<&'a RomFile> for NoMapper {
    fn from(_: &'a RomFile) -> Self { NoMapper {} }
}

type PpuReadFunction = fn(&NROM, u16, &OpenBus) -> PpuReadResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NROM {
    pub prg_ram_size: u16,
    pub prg_ram_battery_backed: bool,
    pub prg_rom_size: u16,
    pub prg_rom: Memory,
    pub prg_ram: Option<Memory>,
    pub chr_mem: Option<Memory>,
    pub nametable_arrangement: NametableArrangement,
    #[serde(skip, default = "NROM::default_lookup_table")]
    ppu_bus_lookup: Box<[PpuReadFunction; u16::MAX as usize + 1]>,
}

impl NROM {
    #[allow(clippy::unwrap_used)]
    fn default_lookup_table() -> Box<[PpuReadFunction; u16::MAX as usize + 1]> {
        vec![NROM::ppu_read_unmapped as PpuReadFunction; u16::MAX as usize + 1]
            .into_boxed_slice()
            .try_into()
            .unwrap()
    }
}

impl MapperLike for NROM {
    #[inline]
    fn write(&mut self, addr: u16, data: u8, _: u64) -> CpuWriteResult {
        match addr {
            0x4020..=0xFFFF => {
                #[allow(clippy::collapsible_if)]
                if (0x6000..=0x7FFF).contains(&addr) {
                    if let Some(prg_ram) = &mut self.prg_ram {
                        let addr = (addr - 0x6000) % self.prg_ram_size;
                        prg_ram.write(u32::from(addr), data);
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
        if (0x4000..=0x4014).contains(&addr) || addr >= 0x4018 {
            let (val, update) = match addr {
                0x6000..=0x7FFF => {
                    if let Some(prg_ram) = &self.prg_ram {
                        let addr = (addr - 0x6000) % self.prg_ram_size;
                        (prg_ram.read(u32::from(addr), open_bus), true)
                    } else {
                        (open_bus.read(), false)
                    }
                }
                0x8000..=0xFFFF => (
                    self.prg_rom
                        .read(u32::from((addr - 0x8000) % self.prg_rom_size), open_bus),
                    true,
                ),
                _ => (open_bus.read(), false),
            };

            return CpuReadResult::Handled(val, update);
        }

        CpuReadResult::Registered
    }

    #[inline(always)]
    fn ppu_read(&mut self, addr: u16, open_bus: &OpenBus) -> PpuReadResult {
        self.ppu_read_debug(addr, open_bus)
    }

    #[inline(always)]
    fn ppu_read_debug(&self, addr: u16, open_bus: &OpenBus) -> PpuReadResult {
        let func = self.ppu_bus_lookup[addr as usize];
        func(self, addr, open_bus)
    }

    #[inline]
    fn ppu_write(&mut self, addr: u16, data: u8) -> PpuWriteResult {
        match addr {
            0..=0x1FFF => {
                if let Some(mem) = &mut self.chr_mem {
                    mem.write(u32::from(addr), data);
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

    fn get_registers_debug(&self) -> MapperRegisterTables {
        let mut state = RegisterMap::new();
        state.insert(
            "name".to_string(),
            RegisterEntry::new(
                RegisterValue::Text("NROM".to_string()),
                RegisterFormat::Text,
            ),
        );
        state.insert(
            "prg_ram_size".to_string(),
            RegisterEntry::new(
                RegisterValue::U16(self.prg_ram_size),
                RegisterFormat::Decimal,
            ),
        );
        state.insert(
            "prg_rom_size".to_string(),
            RegisterEntry::new(
                RegisterValue::U16(self.prg_rom_size),
                RegisterFormat::Decimal,
            ),
        );
        state.insert(
            "prg_ram_battery_backed".to_string(),
            RegisterEntry::new(
                RegisterValue::Bool(self.prg_ram_battery_backed),
                RegisterFormat::Bool,
            ),
        );
        state.insert(
            "nametable_arrangement".to_string(),
            RegisterEntry::new(
                RegisterValue::Text(format!("{:?}", self.nametable_arrangement)),
                RegisterFormat::Text,
            ),
        );

        let mut tables = MapperRegisterTables::new();
        tables.insert("General".to_string(), state);
        tables
    }

    #[inline]
    fn poll_irq(&self) -> bool { false }

    fn build_ppu_map(&mut self) {
        for addr in 0..u16::MAX {
            match addr {
                0..=0x1FFF =>
                {
                    #[allow(clippy::cast_possible_truncation)]
                    if self.chr_mem.is_some() {
                        self.ppu_bus_lookup[addr as usize] = NROM::ppu_read_handled_w_mem;
                    } else {
                        self.ppu_bus_lookup[addr as usize] = NROM::ppu_read_handled;
                    }
                }
                0x2000..=0x3EFF => {
                    self.ppu_bus_lookup[addr as usize] = NROM::ppu_read_nametable;
                }
                _ => self.ppu_bus_lookup[addr as usize] = NROM::ppu_read_unmapped,
            }
        }
    }
}

impl NROM {
    fn ppu_read_handled_w_mem(&self, addr: u16, open_bus: &OpenBus) -> PpuReadResult {
        #[allow(clippy::expect_used)]
        PpuReadResult::Handled(
            self.chr_mem
                .as_ref()
                .expect("Called ppu_read_handled_w_rom without rom")
                .read(u32::from(addr), open_bus),
            false,
        )
    }

    #[allow(clippy::unused_self)]
    #[allow(clippy::cast_possible_truncation)]
    fn ppu_read_handled(&self, addr: u16, _: &OpenBus) -> PpuReadResult {
        PpuReadResult::Handled(addr as u8, false)
    }

    fn ppu_read_nametable(&self, addr: u16, _: &OpenBus) -> PpuReadResult {
        PpuReadResult::Nametable(self.nametable_arrangement.resolve_address(addr))
    }

    #[allow(clippy::unused_self)]
    fn ppu_read_unmapped(&self, _: u16, _: &OpenBus) -> PpuReadResult { PpuReadResult::Registered }
}

impl From<&RomFile> for NROM {
    fn from(rom: &RomFile) -> Self {
        let prg_ram_size = Mapper::get_likely_correct_ram_size(rom);
        let battery_backed = rom.is_battery_backed || rom.prg_memory.prg_nvram_size > 0;

        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::unwrap_used)]
        NROM {
            prg_ram_battery_backed: battery_backed,
            prg_ram_size: prg_ram_size as u16,
            prg_rom_size: rom.prg_memory.prg_rom_size as u16,
            prg_rom: rom.get_prg_rom(),
            chr_mem: rom.get_chr_mem(),
            prg_ram: rom.get_prg_ram(),
            nametable_arrangement: rom.get_nametable_arrangement(),
            ppu_bus_lookup: vec![NROM::ppu_read_unmapped as PpuReadFunction; u16::MAX as usize + 1]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
        }
    }
}
