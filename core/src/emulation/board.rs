use std::convert::Into;
use std::ops::RangeInclusive;

use crate::emulation::cpu::{Cpu, INTERNAL_RAM_SIZE};
use crate::emulation::mapper::{
    CpuReadResult, CpuWriteResult, Mapper, MapperLike, NoMapper, PpuReadResult, PpuWriteResult,
};
use crate::emulation::mem::palette_ram::PaletteRam;
use crate::emulation::mem::{Memory, OpenBus};
use crate::emulation::peripherals::{Peripheral, PeripheralDevice};
use crate::emulation::ppu::{
    Ppu, OPEN_BUS_DECAY_DELAY, PALETTE_RAM_END_ADDRESS, PALETTE_RAM_SIZE,
    PALETTE_RAM_START_ADDRESS, VRAM_SIZE,
};
use crate::emulation::rom::RomFile;
use crate::emulation::savestate::BoardState;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReadResult {
    value: u8,
    update_open_bus: bool,
    mask: u8,
}

impl From<u8> for ReadResult {
    #[inline(always)]
    fn from(value: u8) -> Self {
        ReadResult {
            value,
            update_open_bus: true,
            mask: 0xFF,
        }
    }
}

impl ReadResult {
    #[inline(always)]
    pub fn to_false(mut self) -> Self {
        self.update_open_bus = false;
        self
    }

    #[inline(always)]
    pub fn with_mask(mut self, mask: u8) -> Self {
        self.mask = mask;
        self
    }

    #[inline(always)]
    pub fn with_update(mut self, update: bool) -> Self {
        self.update_open_bus = update;
        self
    }
}

pub struct Board {
    pub cpu: Cpu,
    pub ppu: Ppu,
    pub cpu_ram: Memory,
    pub nametable_ram: Memory,
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
    fn write(&mut self, addr: u16, data: u8, cycle: u128);
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
        let res = self.mapper.read(addr, self.cpu_open_bus);

        let res = match res {
            CpuReadResult::Handled(data, update) => ReadResult::from(data).with_update(update),
            CpuReadResult::Registered => match addr {
                0..=0x1FFF => self.cpu_ram.read(addr as u32, self.cpu_open_bus).into(),
                0x2000..=0x3FFF => self.read_ppu_reg(addr),
                0x4000..=0x401F => self.read_apu_io(addr),
                _ => ReadResult::from(self.cpu_open_bus.read()).to_false(),
            },
        };

        let changed = res.mask != 0xFF;

        if res.update_open_bus {
            self.cpu_open_bus.set_masked(res.value, res.mask);
        }

        if changed {
            self.cpu_open_bus.read()
        } else {
            res.value
        }
    }

    #[inline]
    fn read_debug(&self, addr: u16) -> u8 {
        let res = self.mapper.read_debug(addr, self.cpu_open_bus);

        match res {
            CpuReadResult::Handled(data, _) => data,
            CpuReadResult::Registered => match addr {
                0..=0x1FFF => self.cpu_ram.snapshot(addr as u32, self.cpu_open_bus),
                0x2000..=0x3FFF => self.snapshot_ppu_reg(addr, 0),
                0x4000..=0x401F => self.snapshot_apu_io(addr, self.cpu_open_bus),
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
    fn write(&mut self, addr: u16, data: u8, cycle: u128) {
        let res = self.mapper.write(addr, data, cycle);
        self.cpu_open_bus.set_masked(data, 0xFF);

        match res {
            CpuWriteResult::Handled => {}
            CpuWriteResult::Registered => match addr {
                0..=0x1FFF => {
                    self.cpu_ram.write(addr as u32, data);
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
            CpuWriteResult::Handled => {}

            CpuWriteResult::Registered => {
                if let 0..=0x1FFF = addr {
                    self.cpu_ram.init(addr as u32, data);
                }
            }
        }
    }

    #[inline]
    fn get_ppu_open_bus(&mut self) -> &mut OpenBus { self.ppu_io_bus }

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
        let res = self.mapper.ppu_read(addr, self.ppu_io_bus);

        let res = match res {
            PpuReadResult::Handled(data, update) => ReadResult::from(data).with_update(update),
            PpuReadResult::Nametable(addr) => {
                ReadResult::from(self.nametable_ram.read(addr as u32, self.ppu_io_bus))
                    .to_false()
            }
            PpuReadResult::Registered => match addr {
                0x3F00..=0x3FFF => ReadResult::from(
                    self.palette_ram
                        .read((addr - 0x3F00) % PALETTE_RAM_SIZE, self.ppu_io_bus),
                )
                .to_false(),
                _ => ReadResult::from(self.ppu_io_bus.read()).to_false(),
            },
        };

        let changed = res.mask != 0xFF;

        if res.update_open_bus {
            self.ppu_io_bus.set_masked(res.value, res.mask);
        }

        if changed {
            self.ppu_io_bus.read()
        } else {
            res.value
        }
    }

    #[inline]
    fn read_debug(&self, addr: u16) -> u8 {
        let res = self.mapper.ppu_read_debug(addr, self.ppu_io_bus);

        match res {
            PpuReadResult::Handled(data, _) => data,
            PpuReadResult::Nametable(addr) => self
                .nametable_ram
                .snapshot(addr as u32, self.ppu_io_bus),
            PpuReadResult::Registered => match addr {
                0x3F00..=0x3FFF => self
                    .palette_ram
                    .snapshot((addr - 0x3F00) % PALETTE_RAM_SIZE, self.ppu_io_bus),
                _ => self.ppu_io_bus.read(),
            },
        }
    }

    #[inline]
    fn write(&mut self, addr: u16, data: u8) {
        let res = self.mapper.ppu_write(addr, data);

        match res {
            PpuWriteResult::Handled => {}
            PpuWriteResult::Nametable(addr) => self.nametable_ram.write(addr as u32, data),
            PpuWriteResult::Registered => match addr {
                0x3F00..=0x3FFF => self
                    .palette_ram
                    .write((addr - 0x3F00) % PALETTE_RAM_SIZE, data),
                _ => self.ppu_io_bus.set_masked(data, 0xFF),
            },
        }

        self.ppu_io_bus.set_masked(data, 0xFF);
    }

    #[inline]
    fn init(&mut self, addr: u16, data: u8) {
        let res = self.mapper.ppu_init(addr, data);

        match res {
            PpuWriteResult::Handled => {}
            PpuWriteResult::Nametable(addr) => {
                self.nametable_ram.init(addr as u32, data);
            }
            PpuWriteResult::Registered => {
                if let 0x3F00..=0x3FFF = addr {
                    self.palette_ram
                        .init((addr - 0x3F00) % PALETTE_RAM_SIZE, data)
                }
            }
        }
    }

    #[inline]
    fn get_ppu_open_bus(&self) -> &OpenBus { self.ppu_io_bus }
}

pub struct CpuBusView<'a> {
    mapper: &'a mut Mapper,
    cpu_open_bus: &'a mut OpenBus,
    ppu_io_bus: &'a mut OpenBus,
    cpu_ram: &'a mut Memory,
    nametable_ram: &'a mut Memory,
    palette_ram: &'a mut PaletteRam,
    ppu: &'a mut Ppu,
    irq: &'a mut bool,
    controller1: &'a mut Option<Peripheral>,
    controller2: &'a mut Option<Peripheral>,
    joystick_strobe_data: &'a mut u8,
}

impl<'a> CpuBusView<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn from(
        mapper: &'a mut Mapper,
        cpu_open_bus: &'a mut OpenBus,
        ppu_open_bus: &'a mut OpenBus,
        cpu_ram: &'a mut Memory,
        nametable_ram: &'a mut Memory,
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
            ppu_io_bus: ppu_open_bus,
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
    fn read_ppu_reg(&mut self, addr: u16) -> ReadResult {
        let mut bus = PpuBusView::from(
            self.mapper,
            self.ppu_io_bus,
            self.nametable_ram,
            self.palette_ram,
        );

        match addr % 8 {
            0x2 => {
                self.ppu_io_bus
                    .set_masked(self.ppu.get_ppu_status(), 0b1110_0000);
                self.ppu_io_bus.read().into()
            }
            0x4 => self.ppu.get_oam_at_addr(self.ppu_io_bus).into(),
            0x7 => {
                let val = self.ppu.get_vram_at_addr(&mut bus);

                if (PALETTE_RAM_START_ADDRESS..=PALETTE_RAM_END_ADDRESS)
                    .contains(&self.ppu.v_register)
                {
                    self.ppu_io_bus.set_masked(val, 0b0011_1111);
                } else {
                    self.ppu_io_bus.set_masked(val, 0xFF)
                }

                val.into()
            }
            _ => ReadResult::from(self.ppu_io_bus.read()),
        }
    }

    #[inline]
    fn snapshot_apu_io(&self, addr: u16, open_bus: &OpenBus) -> u8 {
        match addr {
            0x4000..=0x4014 => open_bus.read(),
            0x4016 => {
                if let Some(controller) = &self.controller1 {
                    controller.read_debug()
                } else {
                    open_bus.read()
                }
            }
            0x4017 => {
                if let Some(controller) = &self.controller2 {
                    controller.read_debug()
                } else {
                    open_bus.read()
                }
            }
            0x4018..=0x401F => open_bus.read(),
            _ => open_bus.read(),
        }
    }

    #[inline]
    fn read_apu_io(&mut self, addr: u16) -> ReadResult {
        match addr {
            0x4000..=0x4014 => ReadResult::from(self.cpu_open_bus.read()).to_false(),
            0x4016 => match self.controller1.as_mut() {
                Some(controller) => ReadResult::from(controller.read()).with_mask(!0b11100000),
                None => ReadResult::from(self.cpu_open_bus.read()).to_false(),
            },
            0x4017 => match self.controller2.as_mut() {
                Some(controller) => ReadResult::from(controller.read()).with_mask(!0b11100000),
                None => ReadResult::from(self.cpu_open_bus.read()).to_false(),
            },
            0x4018..=0x401F => ReadResult::from(self.cpu_open_bus.read()).to_false(),
            _ => ReadResult::from(self.cpu_open_bus.read()).to_false(),
        }
    }

    #[inline]
    fn snapshot_ppu_reg(&self, addr: u16, _: u8) -> u8 {
        match addr {
            0x2 => self.ppu.snapshot_ppu_status(),
            0x4 => self.ppu.snapshot_oam_at_addr(self.ppu_io_bus),
            0x7 => self.ppu.snapshot_vram_at_addr(),
            _ => 0,
        }
    }

    #[inline]
    fn write_ppu_reg(&mut self, addr: u16, data: u8) {
        self.ppu_io_bus.set_masked(data, 0xFF);
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
                    self.mapper,
                    self.ppu_io_bus,
                    self.nametable_ram,
                    self.palette_ram,
                );
                self.ppu.write_vram(data, &mut bus);
            }
            _ => (),
        };
    }

    #[inline]
    fn write_apu_io(&mut self, addr: u16, data: u8) {
        #[allow(clippy::single_match)]
        match addr {
            0x4016 => {
                *self.joystick_strobe_data = data & 0b111;
                Board::update_controllers(
                    self.controller1,
                    self.controller2,
                    self.joystick_strobe_data,
                )
            }
            _ => {}
        }
    }
}

pub struct PpuBusView<'a> {
    mapper: &'a mut Mapper,
    ppu_io_bus: &'a mut OpenBus,
    nametable_ram: &'a mut Memory,
    palette_ram: &'a mut PaletteRam,
}

impl<'a> PpuBusView<'a> {
    pub fn from(
        mapper: &'a mut Mapper,
        ppu_open_bus: &'a mut OpenBus,
        nametable_ram: &'a mut Memory,
        palette_ram: &'a mut PaletteRam,
    ) -> PpuBusView<'a> {
        PpuBusView {
            mapper,
            ppu_io_bus: ppu_open_bus,
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
            cpu_ram: Memory::new(INTERNAL_RAM_SIZE as usize, true),
            nametable_ram: Memory::new(VRAM_SIZE as usize, true),
            palette_ram: PaletteRam::default(),
            controller1: None,
            controller2: None,
            joystick_strobe_data: 0,
            mapper,
            irq: false,
        }
    }

    pub fn attach_controllers(
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

    pub fn update_controllers(
        controller1: &mut Option<Peripheral>,
        controller2: &mut Option<Peripheral>,
        joystick_strobe_data: &u8,
    ) {
        if let Some(c1) = controller1 {
            c1.handle_strobe_data(*joystick_strobe_data);
        }
        if let Some(c2) = controller2 {
            c2.handle_strobe_data(*joystick_strobe_data);
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
            cpu_ram: Memory::from((&state.cpu_ram, true)),
            nametable_ram: Memory::from((&state.nametable_ram, true)),
            palette_ram: PaletteRam::from(&state.palette_ram),
            mapper: state.mapper.clone(),
            cpu_open_bus: state.cpu_open_bus,
            ppu_open_bus: state.ppu_open_bus,
            controller1: state.controller1.clone(),
            controller2: state.controller2.clone(),
            joystick_strobe_data: state.joystick_strobe_data,
            irq: false,
        }
    }
}
