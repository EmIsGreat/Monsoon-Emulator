use std::convert::Into;
use std::ops::RangeInclusive;

use crate::emulation::apu::{Apu, EnvelopeUnit, LengthCounter, SweepUnit};
use crate::emulation::cpu::{Cpu, INTERNAL_RAM_SIZE};
use crate::emulation::mapper::{
    CpuReadResult, CpuWriteResult, Mapper, MapperLike, NoMapper, PpuReadResult, PpuWriteResult,
};
use crate::emulation::mem::palette_ram::PaletteRam;
use crate::emulation::mem::{Memory, OpenBus};
use crate::emulation::peripherals::{Peripheral, PeripheralDevice};
use crate::emulation::ppu::{
    OPEN_BUS_DECAY_DELAY, PALETTE_RAM_END_ADDRESS, PALETTE_RAM_SIZE, PALETTE_RAM_START_ADDRESS,
    Ppu, VRAM_SIZE,
};
use crate::emulation::rom::RomFile;

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
    pub apu: Apu,
    pub cpu_ram: Memory,
    pub nametable_ram: Memory,
    pub palette_ram: PaletteRam,
    pub mapper: Mapper,
    pub cpu_open_bus: OpenBus,
    pub ppu_open_bus: OpenBus,
    pub port1: Option<Peripheral>,
    pub port2: Option<Peripheral>,
    pub joystick_strobe_data: u8,
    pub irq: bool,
}

#[allow(unused_attributes)]
pub trait CpuBus {
    fn read(&mut self, addr: u16) -> u8;
    fn read_debug(&self, addr: u16) -> u8;
    fn get_range(&self, addr: RangeInclusive<u16>) -> Vec<u8>;
    fn write(&mut self, addr: u16, data: u8, cycle: u64);
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

impl CpuBus for CpuBusView<'_> {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        let res = self.mapper.read(addr, self.cpu_open_bus);

        let res = match res {
            CpuReadResult::Handled(data, update) => ReadResult::from(data).with_update(update),
            CpuReadResult::Registered => match addr {
                0..=0x1FFF => self.cpu_ram.read(u32::from(addr), self.cpu_open_bus).into(),
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
                0..=0x1FFF => self.cpu_ram.snapshot(u32::from(addr), self.cpu_open_bus),
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
    fn write(&mut self, addr: u16, data: u8, cycle: u64) {
        let res = self.mapper.write(addr, data, cycle);
        self.cpu_open_bus.set_masked(data, 0xFF);

        match res {
            CpuWriteResult::Handled => {}
            CpuWriteResult::Registered => match addr {
                0..=0x1FFF => {
                    self.cpu_ram.write(u32::from(addr), data);
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
                    self.cpu_ram.init(u32::from(addr), data);
                }
            }
        }
    }

    #[inline]
    fn get_ppu_open_bus(&mut self) -> &mut OpenBus { self.ppu_io_bus }

    #[inline]
    fn poll_nmi(&mut self) -> bool { self.ppu.poll_nmi() }

    #[inline]
    fn poll_irq(&mut self) -> bool { self.mapper.poll_irq() || self.apu.poll_irq() }

    #[inline]
    fn set_irq(&mut self, val: bool) { *self.irq = val }
}

impl PpuBus for PpuBusView<'_> {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        let res = self.mapper.ppu_read(addr, self.ppu_io_bus);

        let res = match res {
            PpuReadResult::Handled(data, update) => ReadResult::from(data).with_update(update),
            PpuReadResult::Nametable(addr) => {
                ReadResult::from(self.nametable_ram.read(u32::from(addr), self.ppu_io_bus))
                    .to_false()
            }
            PpuReadResult::Registered if (0x3F00..=0x3FFF).contains(&addr) => {
                let val = self
                    .palette_ram
                    .read((addr - 0x3F00) % PALETTE_RAM_SIZE, self.ppu_io_bus);

                // Zeroes the four low bits in case grayscale is enabled
                let mask = !(u8::from(self.grayscale_enabled).wrapping_neg() & 0x0F);

                ReadResult::from(val & mask).to_false()
            }
            #[allow(clippy::cast_possible_truncation)]
            PpuReadResult::Registered => ReadResult::from(addr as u8).to_false(),
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
                .snapshot(u32::from(addr), self.ppu_io_bus),
            PpuReadResult::Registered => match addr {
                0x3F00..=0x3FFF => {
                    self
                        .palette_ram
                        .snapshot((addr - 0x3F00) % PALETTE_RAM_SIZE, self.ppu_io_bus)
                        // Zeroes the four low bits in case grayscale is enabled
                        & !(u8::from(self.grayscale_enabled).wrapping_neg() & 0x0F)
                }
                _ => self.ppu_io_bus.read(),
            },
        }
    }

    #[inline]
    fn write(&mut self, addr: u16, data: u8) {
        let res = self.mapper.ppu_write(addr, data);

        match res {
            PpuWriteResult::Handled => {}
            PpuWriteResult::Nametable(addr) => self.nametable_ram.write(u32::from(addr), data),
            #[allow(clippy::single_match)]
            PpuWriteResult::Registered => match addr {
                0x3F00..=0x3FFF => {
                    let prev = if self.grayscale_enabled {
                        self.palette_ram.read(addr, self.ppu_io_bus) & 0x0F
                    } else {
                        0
                    };

                    self.palette_ram.write(
                        (addr - 0x3F00) % PALETTE_RAM_SIZE,
                        data
                            // Zeroes the four low bits in case grayscale is enabled, and then
                            // or's in the previous palette ram value. Effectively ignores the
                            // lower four bits completely in case of grayscale enable
                            & (!(u8::from(self.grayscale_enabled).wrapping_neg() & 0x0F)
                        ) | prev,
                    );
                }
                _ => {}
            },
        }
    }

    #[inline]
    fn init(&mut self, addr: u16, data: u8) {
        let res = self.mapper.ppu_init(addr, data);

        match res {
            PpuWriteResult::Handled => {}
            PpuWriteResult::Nametable(addr) => {
                self.nametable_ram.init(u32::from(addr), data);
            }
            PpuWriteResult::Registered => {
                if let 0x3F00..=0x3FFF = addr {
                    self.palette_ram
                        .init((addr - 0x3F00) % PALETTE_RAM_SIZE, data);
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
    apu: &'a mut Apu,
    irq: &'a mut bool,
    controller1: &'a mut Option<Peripheral>,
    controller2: &'a mut Option<Peripheral>,
    strobe_data: &'a mut u8,
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
        apu: &'a mut Apu,
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
            apu,
            irq,
            controller1,
            controller2,
            strobe_data: joystick_probe_data,
        }
    }

    #[inline]
    fn read_ppu_reg(&mut self, addr: u16) -> ReadResult {
        let grayscale = self.ppu.get_grayscale_enabled();

        let mut bus = PpuBusView::from(
            self.mapper,
            self.ppu_io_bus,
            self.nametable_ram,
            self.palette_ram,
            grayscale,
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
                    self.ppu_io_bus.set_masked(val, 0xFF);
                }

                val.into()
            }
            _ => ReadResult::from(self.ppu_io_bus.read()),
        }
    }

    #[inline]
    fn snapshot_apu_io(&self, addr: u16, open_bus: &OpenBus) -> u8 {
        match addr {
            0x4015 => {
                let frame_interrupt = if self.apu.frame_counter.frame_interrupt {
                    0b0100_0000
                } else {
                    0
                };

                frame_interrupt | (self.cpu_open_bus.read() & 0b0010_0000)
            }
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
            _ => open_bus.read(),
        }
    }

    #[inline]
    fn read_apu_io(&mut self, addr: u16) -> ReadResult {
        match addr {
            0x4015 => {
                let frame_interrupt = if self.apu.frame_counter.get_frame_interrupt_for_register() {
                    1u8
                } else {
                    0
                };

                let dmc_interrupt = if self.apu.dmc_unit.irq_asserted {
                    1u8
                } else {
                    0
                };

                let p1_halt_status = get_len_halt_status_bit(&self.apu.pulse_1.length_counter);
                let p2_halt_status = get_len_halt_status_bit(&self.apu.pulse_2.length_counter);
                let triangle_halt_status =
                    get_len_halt_status_bit(&self.apu.triangle_generator.length_counter);
                let noise_halt_status =
                    get_len_halt_status_bit(&self.apu.noise_generator.length_counter);

                let dmc_bytes_remaining = if self.apu.dmc_unit.bytes_remaining != 0 {
                    1
                } else {
                    0
                };

                ReadResult::from(
                    dmc_interrupt << 7
                        | frame_interrupt << 6
                        | (self.cpu_open_bus.read() & 0b0010_0000)
                        | dmc_bytes_remaining << 4
                        | noise_halt_status << 3
                        | triangle_halt_status << 2
                        | p2_halt_status << 1
                        | p1_halt_status,
                )
                .to_false()
            }
            0x4016 => match self.controller1 {
                Some(controller) => ReadResult::from(controller.read()).with_mask(!0b1110_0000),
                None => ReadResult::from(self.cpu_open_bus.read()).to_false(),
            },
            0x4017 => match self.controller2 {
                Some(controller) => ReadResult::from(controller.read()).with_mask(!0b1110_0000),
                None => ReadResult::from(self.cpu_open_bus.read()).to_false(),
            },
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
                let grayscale = self.ppu.get_grayscale_enabled();

                let mut bus = PpuBusView::from(
                    self.mapper,
                    self.ppu_io_bus,
                    self.nametable_ram,
                    self.palette_ram,
                    grayscale,
                );

                self.ppu.write_vram(data, &mut bus);
            }
            _ => (),
        }
    }

    #[inline]
    fn write_apu_io(&mut self, addr: u16, data: u8) {
        match addr {
            0x4000 => {
                reload_envelope(&mut self.apu.pulse_1.envelope, data);
                let halt = (data & 0b10_0000) >> 5 == 1;
                self.apu.pulse_1.length_counter.halt = halt;
                self.apu.pulse_1.duty_ratio = data >> 6;
            }
            0x4001 => {
                reload_sweep(&mut self.apu.pulse_1.sweep_unit, data);
            }
            0x4002 => {
                self.apu.pulse_1.period = (self.apu.pulse_1.period & 0xFF) | data as u16;
            }
            0x4003 => {
                load_length_counter(&mut self.apu.pulse_1.length_counter, data);
                restart_envelope(&mut self.apu.pulse_1.envelope);
                self.apu.pulse_1.phase = 0;
                self.apu.pulse_1.period =
                    (self.apu.pulse_1.period & 0xFF00) | (((data & 0b1110_0000) as u16) << 3)
            }
            0x4004 => {
                reload_envelope(&mut self.apu.pulse_2.envelope, data);
                let halt = (data & 0b10_0000) >> 5 == 1;
                self.apu.pulse_2.length_counter.halt = halt;
                self.apu.pulse_2.duty_ratio = data >> 6;
            }
            0x4005 => {
                reload_sweep(&mut self.apu.pulse_2.sweep_unit, data);
            }
            0x4006 => {
                self.apu.pulse_1.period = (self.apu.pulse_1.period & 0xFF) | u16::from(data);
            }
            0x4007 => {
                load_length_counter(&mut self.apu.pulse_2.length_counter, data);
                restart_envelope(&mut self.apu.pulse_2.envelope);
                self.apu.pulse_2.phase = 0;
                self.apu.pulse_1.period =
                    (self.apu.pulse_2.period & 0xFF00) | (((data & 0b1110_0000) as u16) << 3);
            }
            0x4008 => {
                let control = (data >> 7) == 1;
                let reload = data & !0x80;

                self.apu.triangle_generator.length_counter.halt = control;
                self.apu.triangle_generator.linear_counter.control_flag = control;
                self.apu.triangle_generator.linear_counter.reload_value = reload;
            }
            0x400A => {
                self.apu.triangle_generator.period =
                    (self.apu.triangle_generator.period & 0xFF) | data as u16;
            }
            0x400B => {
                load_length_counter(&mut self.apu.triangle_generator.length_counter, data);
                self.apu.triangle_generator.linear_counter.reload_flag = true;
                self.apu.triangle_generator.period = (self.apu.triangle_generator.period & 0xFF00)
                    | (((data & 0b1110_0000) as u16) << 3);
            }
            0x400C => {
                reload_envelope(&mut self.apu.noise_generator.envelope, data);
                restart_envelope(&mut self.apu.noise_generator.envelope);
                let halt = (data & 0b10_0000) >> 5 == 1;
                self.apu.noise_generator.length_counter.halt = halt;
            }
            0x400E => {
                let noise_mode = (data >> 7) == 1;
                let noise_period = data & 0b1111;

                self.apu.noise_generator.noise_mode = noise_mode;
                self.apu.noise_generator.set_period(noise_period);
            }
            0x400F => {
                load_length_counter(&mut self.apu.noise_generator.length_counter, data);
            }
            0x4010 => {
                let irq_enable = (data >> 7) == 1;
                let does_loop = (data >> 6) == 1;
                let sample_rate = data & 0b1111;

                self.apu.dmc_unit.sample_end_irq_enable = irq_enable;
                self.apu.dmc_unit.does_loop = does_loop;
                self.apu.dmc_unit.sample_rate = sample_rate;
            }
            0x4011 => {
                let load_counter = data & (!0x80);
                self.apu.dmc_unit.load_counter = load_counter;
            }
            0x4012 => {
                self.apu.dmc_unit.sample_address = data;
            }
            0x4013 => {
                self.apu.dmc_unit.sample_length = data;
            }
            0x4015 => {
                let p1_enable = (data & 0b1) == 0b1;
                let p2_enable = (data & 0b10) == 0b10;
                let triangle_enable = (data & 0b100) == 0b100;
                let noise_enable = (data & 0b1000) == 0b1000;

                self.apu.pulse_1.length_counter.set_enabled(p1_enable);
                self.apu.pulse_2.length_counter.set_enabled(p2_enable);
                self.apu
                    .triangle_generator
                    .length_counter
                    .set_enabled(triangle_enable);
                self.apu
                    .noise_generator
                    .length_counter
                    .set_enabled(noise_enable);

                let dmc_bit = (data >> 4) == 1;
                if dmc_bit {
                    self.apu.dmc_unit.bytes_remaining = 0;
                } else if self.apu.dmc_unit.bytes_remaining == 0 {
                    self.apu.dmc_unit.restart_sample_playback();
                }

                self.apu.dmc_unit.irq_pending = false;
            }
            0x4016 => {
                *self.strobe_data = data & 1;
                Board::update_controllers(self.controller1, self.controller2, *self.strobe_data);
            }
            0x4017 => {
                self.apu.frame_counter.external_sync = data & 0x80 != 0;
                self.apu.frame_counter.interrupt_inhibit = data & 0x40 != 0;
                self.apu.frame_counter.frame_interrupt = !self.apu.frame_counter.interrupt_inhibit;
            }
            _ => {}
        }
    }
}

fn reload_envelope(env: &mut EnvelopeUnit, data: u8) {
    let reload_val = data & 0b1111;
    let const_vol = ((data & 0b1_0000) >> 4) == 1;
    let does_loop = ((data & 0b10_0000) >> 5) == 1;
    env.divider_reload = reload_val;
    env.const_volume = const_vol;
    env.does_loop = does_loop;
    env.start = true;
}

fn reload_sweep(sweep: &mut SweepUnit, data: u8) {
    let shift_count = data & 0b111;
    let negate = ((data >> 3) & 1) == 1;
    let counter_reload = (data >> 4) & 0b111;
    let sweep_enable = ((data >> 7) & 1) == 1;

    sweep.shift_bits = shift_count;
    sweep.negate = negate;
    sweep.counter_reload = counter_reload;
    sweep.enable = sweep_enable;
    sweep.reload = true;
}

fn restart_envelope(env: &mut EnvelopeUnit) { env.start = true; }

fn load_length_counter(counter: &mut LengthCounter, data: u8) {
    let index = (data & (!0b111)) >> 3;
    counter.load(index);
}

fn get_len_halt_status_bit(len: &LengthCounter) -> u8 {
    u8::from(!len.halt && len.get_counter() != 0)
}

pub struct PpuBusView<'a> {
    mapper: &'a mut Mapper,
    ppu_io_bus: &'a mut OpenBus,
    nametable_ram: &'a mut Memory,
    palette_ram: &'a mut PaletteRam,
    grayscale_enabled: bool,
}

impl<'a> PpuBusView<'a> {
    pub fn from(
        mapper: &'a mut Mapper,
        ppu_io_bus: &'a mut OpenBus,
        nametable_ram: &'a mut Memory,
        palette_ram: &'a mut PaletteRam,
        grayscale_enabled: bool,
    ) -> PpuBusView<'a> {
        PpuBusView {
            mapper,
            ppu_io_bus,
            nametable_ram,
            palette_ram,
            grayscale_enabled,
        }
    }
}

impl Board {
    pub fn new(cpu: Cpu, ppu: Ppu, apu: Apu, mapper: Mapper) -> Board {
        let mut board = Board {
            cpu,
            ppu,
            apu,
            cpu_open_bus: OpenBus::new(OPEN_BUS_DECAY_DELAY),
            ppu_open_bus: OpenBus::new(OPEN_BUS_DECAY_DELAY),
            cpu_ram: Memory::new(INTERNAL_RAM_SIZE as usize, true),
            nametable_ram: Memory::new(VRAM_SIZE as usize, true),
            palette_ram: PaletteRam::default(),
            port1: None,
            port2: None,
            joystick_strobe_data: 0,
            mapper,
            irq: false,
        };

        board.mapper.build_ppu_map();
        board
    }

    pub fn attach_controllers(&mut self, port1: Option<Peripheral>, port2: Option<Peripheral>) {
        self.port1 = port1;
        self.port2 = port2;

        Board::update_controllers(&mut self.port1, &mut self.port2, self.joystick_strobe_data);
    }

    pub fn update_controllers(
        controller1: &mut Option<Peripheral>,
        controller2: &mut Option<Peripheral>,
        joystick_strobe_data: u8,
    ) {
        if let Some(c1) = controller1 {
            c1.handle_strobe_data(joystick_strobe_data);
        }
        if let Some(c2) = controller2 {
            c2.handle_strobe_data(joystick_strobe_data);
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
        self.ppu.reset();
    }

    pub fn load_rom(&mut self, rom_file: &RomFile) {
        self.mapper = rom_file.into();
        self.mapper.build_ppu_map();
    }
}

impl Default for Board {
    fn default() -> Self {
        Board::new(
            Cpu::default(),
            Ppu::default(),
            Apu::default(),
            Mapper::NoMapper(NoMapper {}),
        )
    }
}
