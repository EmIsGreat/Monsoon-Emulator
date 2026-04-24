#![allow(unused_attributes)]

use std::convert::Into;
use std::ops::RangeInclusive;

use crate::emulation::cpu::{Cpu, INTERNAL_RAM_SIZE};
use crate::emulation::mapper::{
    CpuReadResult, CpuWriteResult, Mapper, MapperLike, NoMapper, PpuReadResult, PpuWriteResult,
};
use crate::emulation::mem::palette_ram::PaletteRam;
use crate::emulation::mem::{MemoryDevice, OpenBus, Ram};
use crate::emulation::peripherals::{Peripheral, PeripheralDevice};
use crate::emulation::ppu::{
    Ppu, OPEN_BUS_DECAY_DELAY, PALETTE_RAM_END_ADDRESS, PALETTE_RAM_START_ADDRESS, VRAM_SIZE,
};
use crate::emulation::rom::RomFile;
use crate::emulation::savestate::BoardState;

pub struct Board {
    pub cpu: Cpu,
    pub ppu: Ppu,
    pub cpu_ram: Ram,
    pub nametable_ram: Ram,
    pub palette_ram: PaletteRam,
    pub mapper: Mapper,
    pub cpu_open_bus: OpenBus,
    pub ppu_open_bus: OpenBus,
    pub controller1: Option<Peripheral>,
    pub controller2: Option<Peripheral>,
    pub joystick_strobe_data: u8,
    pub irq: bool,
}

#[allow(unused_attributes)]
pub trait CpuBus {
    fn read(&mut self, addr: u16) -> u8;
    fn read_debug(&self, addr: u16) -> u8;
    fn get_range(&self, addr: RangeInclusive<u16>) -> Vec<u8>;
    fn write(&mut self, addr: u16, data: u8);
    fn init(&mut self, addr: u16, data: u8);
    fn get_ppu_open_bus(&mut self) -> &mut OpenBus;
    fn poll_nmi(&mut self) -> bool;
    fn poll_irq(&mut self) -> bool;
    fn set_irq(&mut self, val: bool);
}

pub trait PpuBus {
    fn read(&mut self, addr: u16) -> u8;
    fn read_debug(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, data: u8);
    fn init(&mut self, addr: u16, data: u8);
    fn get_ppu_open_bus(&self) -> &OpenBus;
}

impl<'a> CpuBus for CpuBusView<'a> {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        let res = self.mapper.read(addr, &self.cpu_open_bus);

        match res {
            CpuReadResult::Handled(data) => data,
            CpuReadResult::Registered => match addr {
                0..=0x1FFF => self.cpu_ram.read(addr, &mut self.cpu_open_bus),
                0x2000..=0x3FFF => self.read_ppu_reg(addr, 0),
                0x4000..=0x401F => CpuBusView::read_apu_io(
                    addr,
                    &mut self.controller1,
                    &mut self.controller2,
                    &self.cpu_open_bus,
                ),
                _ => self.cpu_open_bus.read(),
            },
        }
    }

    #[inline]
    fn read_debug(&self, addr: u16) -> u8 {
        let res = self.mapper.read_debug(addr, &self.cpu_open_bus);

        match res {
            CpuReadResult::Handled(data) => data,
            CpuReadResult::Registered => match addr {
                0..=0x1FFF => self.cpu_ram.snapshot(addr, &self.cpu_open_bus),
                0x2000..=0x3FFF => self.snapshot_ppu_reg(addr, 0),
                0x4000..=0x401F => self.snapshot_apu_io(addr, &self.cpu_open_bus),
                _ => self.cpu_open_bus.read(),
            },
        }
    }

    #[inline]
    fn get_range(&self, addr: RangeInclusive<u16>) -> Vec<u8> {
        let mut vec = Vec::with_capacity(addr.clone().len());
        addr.for_each(|a| vec.push(CpuBus::read_debug(self, a)));
        vec
    }

    #[inline]
    fn write(&mut self, addr: u16, data: u8) {
        let res = self.mapper.write(addr, data);

        match res {
            CpuWriteResult::Handled => {
                return;
            }

            CpuWriteResult::Registered => match addr {
                0..=0x1FFF => {
                    self.cpu_ram.write(addr, data);
                }
                0x2000..=0x3FFF => {
                    self.write_ppu_reg(addr, data);
                }
                0x4000..=0x401F => self.write_apu_io(addr, data),
                _ => {}
            },
        }
    }

    #[inline]
    fn init(&mut self, addr: u16, data: u8) {
        let res = self.mapper.init(addr, data);

        match res {
            CpuWriteResult::Handled => {
                return;
            }

            CpuWriteResult::Registered => match addr {
                0..=0x1FFF => {
                    self.cpu_ram.init(addr, data);
                }
                _ => {}
            },
        }
    }

    #[inline]
    fn get_ppu_open_bus(&mut self) -> &mut OpenBus { &mut self.ppu_open_bus }

    #[inline]
    fn poll_nmi(&mut self) -> bool { self.ppu.poll_nmi() }

    #[inline]
    fn poll_irq(&mut self) -> bool { *self.irq }

    #[inline]
    fn set_irq(&mut self, val: bool) { *self.irq = val }
}

impl<'a> PpuBus for PpuBusView<'a> {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        let res = self.mapper.ppu_read(addr, &self.cpu_open_bus);

        match res {
            PpuReadResult::Handled(data) => data,
            PpuReadResult::Registered => self.ppu_open_bus.read(),
            PpuReadResult::Nametable(addr) => self.nametable_ram.read(addr, &mut self.ppu_open_bus),
            PpuReadResult::Palette(addr) => self.palette_ram.read(addr, &mut self.ppu_open_bus),
        }
    }

    #[inline]
    fn read_debug(&self, addr: u16) -> u8 {
        let res = self.mapper.ppu_read_debug(addr, &self.cpu_open_bus);

        match res {
            PpuReadResult::Handled(data) => data,
            PpuReadResult::Registered => self.ppu_open_bus.read(),
            PpuReadResult::Nametable(addr) => self.nametable_ram.snapshot(addr, &self.ppu_open_bus),
            PpuReadResult::Palette(addr) => self.palette_ram.snapshot(addr, &self.ppu_open_bus),
        }
    }

    #[inline]
    fn write(&mut self, addr: u16, data: u8) {
        let res = self.mapper.ppu_write(addr, data);

        match res {
            PpuWriteResult::Handled => {}
            PpuWriteResult::Nametable(addr) => self.nametable_ram.write(addr, data),
            PpuWriteResult::Palette(addr) => self.palette_ram.write(addr, data),
            PpuWriteResult::Registered => self.ppu_open_bus.set_masked(data, 0xFF),
        }
    }

    #[inline]
    fn init(&mut self, addr: u16, data: u8) {
        let res = self.mapper.ppu_init(addr, data);

        match res {
            PpuWriteResult::Handled => {}
            PpuWriteResult::Nametable(addr) => {
                self.nametable_ram.init(addr, data);
            }
            PpuWriteResult::Palette(addr) => self.palette_ram.init(addr, data),
            PpuWriteResult::Registered => {}
        }
    }

    #[inline]
    fn get_ppu_open_bus(&self) -> &OpenBus { &self.ppu_open_bus }
}

pub struct CpuBusView<'a> {
    mapper: &'a mut Mapper,
    cpu_open_bus: &'a mut OpenBus,
    ppu_open_bus: &'a mut OpenBus,
    cpu_ram: &'a mut Ram,
    nametable_ram: &'a mut Ram,
    palette_ram: &'a mut PaletteRam,
    ppu: &'a mut Ppu,
    irq: &'a mut bool,
    controller1: &'a mut Option<Peripheral>,
    controller2: &'a mut Option<Peripheral>,
    joystick_strobe_data: &'a mut u8,
}

impl<'a> CpuBusView<'a> {
    pub fn from(
        mapper: &'a mut Mapper,
        cpu_open_bus: &'a mut OpenBus,
        ppu_open_bus: &'a mut OpenBus,
        cpu_ram: &'a mut Ram,
        nametable_ram: &'a mut Ram,
        palette_ram: &'a mut PaletteRam,
        ppu: &'a mut Ppu,
        irq: &'a mut bool,
        controller1: &'a mut Option<Peripheral>,
        controller2: &'a mut Option<Peripheral>,
        joystick_probe_data: &'a mut u8,
    ) -> CpuBusView<'a> {
        CpuBusView {
            mapper,
            cpu_open_bus,
            ppu_open_bus,
            cpu_ram,
            nametable_ram,
            palette_ram,
            ppu,
            irq,
            controller1,
            controller2,
            joystick_strobe_data: joystick_probe_data,
        }
    }

    #[inline]
    fn read_ppu_reg(&mut self, addr: u16, _: u8) -> u8 {
        let mut bus = PpuBusView::from(
            &mut self.mapper,
            &mut self.cpu_open_bus,
            &mut self.ppu_open_bus,
            &mut self.nametable_ram,
            &mut self.palette_ram,
        );

        match addr % 8 {
            0x2 => {
                let val = self.ppu.get_ppu_status();
                bus.ppu_open_bus.set_masked(val, 0b1110_0000);
            }
            0x4 => {
                let val = self.ppu.get_oam_at_addr(bus.ppu_open_bus);
                bus.ppu_open_bus.set_masked(val, 0xFF);
            }
            0x7 => {
                let val = self.ppu.get_vram_at_addr(&mut bus);

                match self.ppu.v_register {
                    PALETTE_RAM_START_ADDRESS..=PALETTE_RAM_END_ADDRESS => {
                        bus.ppu_open_bus.set_masked(val, 0b0011_1111);
                    }
                    _ => {
                        bus.ppu_open_bus.set_masked(val, 0xFF);
                    }
                }
            }
            _ => {}
        };

        bus.ppu_open_bus.read()
    }

    #[inline]
    fn snapshot_apu_io(&self, addr: u16, open_bus: &OpenBus) -> u8 {
        match addr {
            0x4000..=0x4014 => open_bus.read(),
            0x4016 => {
                if let Some(controller) = &self.controller1 {
                    controller.read_debug(open_bus)
                } else {
                    open_bus.read()
                }
            }
            0x4017 => {
                if let Some(controller) = &self.controller2 {
                    controller.read_debug(open_bus)
                } else {
                    open_bus.read()
                }
            }
            0x4018..=0x401F => open_bus.read(),
            _ => open_bus.read(),
        }
    }

    #[inline]
    fn read_apu_io(
        addr: u16,
        mut controller1: &mut Option<Peripheral>,
        mut controller2: &mut Option<Peripheral>,
        open_bus: &OpenBus,
    ) -> u8 {
        match addr {
            0x4000..=0x4014 => open_bus.read(),
            0x4016 => {
                if let Some(controller) = &mut controller1 {
                    controller.read(open_bus)
                } else {
                    open_bus.read()
                }
            }
            0x4017 => {
                if let Some(controller) = &mut controller2 {
                    controller.read(open_bus)
                } else {
                    open_bus.read()
                }
            }
            0x4018..=0x401F => open_bus.read(),
            _ => open_bus.read(),
        }
    }

    #[inline]
    fn snapshot_ppu_reg(&self, addr: u16, _: u8) -> u8 {
        match addr {
            0x2 => self.ppu.snapshot_ppu_status(),
            0x4 => self.ppu.snapshot_oam_at_addr(&self.ppu_open_bus),
            0x7 => self.ppu.snapshot_vram_at_addr(),
            _ => 0,
        }
    }

    #[inline]
    fn write_ppu_reg(&mut self, addr: u16, data: u8) {
        self.ppu_open_bus.set_masked(data, 0xFF);
        match addr % 8 {
            0x0 => {
                self.ppu.set_ppu_ctrl(data);
            }
            0x1 => {
                self.ppu.set_mask_register(data);
            }
            0x3 => {
                self.ppu.set_oam_addr_register(data);
            }
            0x4 => {
                self.ppu.write_oam(data);
            }
            0x5 => {
                self.ppu.write_ppu_scroll(data);
            }
            0x6 => {
                self.ppu.write_vram_addr(data);
            }
            0x7 => {
                let mut bus = PpuBusView::from(
                    &mut self.mapper,
                    &mut self.cpu_open_bus,
                    &mut self.ppu_open_bus,
                    &mut self.nametable_ram,
                    &mut self.palette_ram,
                );
                self.ppu.write_vram(data, &mut bus);
            }
            _ => (),
        };
    }

    #[inline]
    fn write_apu_io(&mut self, addr: u16, data: u8) {
        match addr {
            0x4016 => {
                *self.joystick_strobe_data = data & 0b111;
                Board::update_controllers(
                    &mut self.controller1,
                    &mut self.controller2,
                    &self.joystick_strobe_data,
                )
            }
            _ => {}
        }
    }
}

pub struct PpuBusView<'a> {
    mapper: &'a mut Mapper,
    cpu_open_bus: &'a mut OpenBus,
    ppu_open_bus: &'a mut OpenBus,
    nametable_ram: &'a mut Ram,
    palette_ram: &'a mut PaletteRam,
}

impl<'a> PpuBusView<'a> {
    pub fn from(
        mapper: &'a mut Mapper,
        cpu_open_bus: &'a mut OpenBus,
        ppu_open_bus: &'a mut OpenBus,
        nametable_ram: &'a mut Ram,
        palette_ram: &'a mut PaletteRam,
    ) -> PpuBusView<'a> {
        PpuBusView {
            mapper,
            cpu_open_bus,
            ppu_open_bus,
            nametable_ram,
            palette_ram,
        }
    }
}

impl Board {
    pub fn new(cpu: Cpu, ppu: Ppu, mapper: Mapper) -> Board {
        Board {
            cpu,
            ppu,
            cpu_open_bus: OpenBus::new(OPEN_BUS_DECAY_DELAY),
            ppu_open_bus: OpenBus::new(OPEN_BUS_DECAY_DELAY),
            cpu_ram: Ram::new(INTERNAL_RAM_SIZE as usize),
            nametable_ram: Ram::new(VRAM_SIZE),
            palette_ram: PaletteRam::default(),
            controller1: None,
            controller2: None,
            joystick_strobe_data: 0,
            mapper,
            irq: false,
        }
    }

    pub fn attack_controllers(
        &mut self,
        controller1: Option<Peripheral>,
        controller2: Option<Peripheral>,
    ) {
        self.controller1 = controller1;
        self.controller2 = controller2;

        Board::update_controllers(
            &mut self.controller1,
            &mut self.controller2,
            &self.joystick_strobe_data,
        )
    }

    fn update_controllers(
        mut controller1: &mut Option<Peripheral>,
        mut controller2: &mut Option<Peripheral>,
        joystick_strobe_data: &u8,
    ) {
        if let (Some(controller1), Some(controller2)) = (&mut controller1, &mut controller2) {
            controller1.handle_strobe_data(*joystick_strobe_data);
            controller2.handle_strobe_data(*joystick_strobe_data);
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
        self.ppu.reset()
    }

    pub fn load_rom(&mut self, rom_file: &RomFile) { self.mapper = rom_file.into() }
}

impl Default for Board {
    fn default() -> Self { Board::new(Cpu::new(), Ppu::default(), Mapper::NoMapper(NoMapper {})) }
}

impl From<&BoardState> for Board {
    fn from(state: &BoardState) -> Self {
        Board {
            cpu: Cpu::from(&state.cpu),
            ppu: Ppu::from(&state.ppu),
            cpu_ram: Ram::from(&state.cpu_ram),
            nametable_ram: Ram::from(&state.nametable_ram),
            palette_ram: PaletteRam::from(&state.palette_ram),
            mapper: state.mapper.clone(),
            cpu_open_bus: state.cpu_open_bus,
            ppu_open_bus: state.ppu_open_bus,
            controller1: None,
            controller2: None,
            joystick_strobe_data: 0,
            irq: false,
        }
    }
}
