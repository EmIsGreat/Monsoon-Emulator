use serde::{Deserialize, Serialize};

const FRAME_COUNTER_STEP_1: u16 = 3728;
const FRAME_COUNTER_STEP_2: u16 = 7456;
const FRAME_COUNTER_STEP_3: u16 = 11185;
const FRAME_COUNTER_STEP_4: u16 = 14914;
const FRAME_COUNTER_STEP_5: u16 = 18640;
const LENGTH_COUNTER_TABLE: [u8; 0x20] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

const NOISE_TABLE: [u16; 0x10] = [
    2, 4, 8, 16, 32, 48, 64, 80, 101, 127, 190, 254, 391, 508, 1017, 2034,
];

#[derive(Debug, Clone)]
pub struct Apu {
    pub frame_counter: FrameCounter,
    pub pulse_1: PulseGenerator,
    pub pulse_2: PulseGenerator,
    pub noise_generator: NoiseGenerator,
    pub triangle_generator: TriangleGenerator,
    pub dmc_unit: DMCUnit
}

impl Default for Apu {
    fn default() -> Self {
        let mut pulse_1 = PulseGenerator::default();
        pulse_1.sweep_unit.ones_complement = true;

        Self {
            frame_counter: Default::default(),
            pulse_1,
            pulse_2: Default::default(),
            noise_generator: Default::default(),
            triangle_generator: Default::default(),
        }
    }
}

impl Apu {
    #[inline(always)]
    pub fn clock_frame_counter(&mut self, is_proper: bool) {
        if let Some(res) = self.frame_counter.clock(is_proper) {
            match res {
                FrameCounterClockResult::HalfFrame => self.clock_half_frame(),
                FrameCounterClockResult::QuarterFrame => self.clock_quarter_frame(),
            }
        }
    }

    #[inline]
    pub fn clock_half_frame(&mut self) {
        self.clock_quarter_frame();
        self.pulse_1.length_counter.clock();
        self.pulse_2.length_counter.clock();
        self.triangle_generator.length_counter.clock();
        self.noise_generator.length_counter.clock();

        match self.pulse_1.sweep_unit.clock(self.pulse_1.period) {
            Ok(p) => {
                self.pulse_1.period = p;
            }
            Err(m) => {
                self.pulse_1.muted = m;
            }
        }

        match self.pulse_2.sweep_unit.clock(self.pulse_2.period) {
            Ok(p) => {
                self.pulse_2.period = p;
            }
            Err(m) => {
                self.pulse_2.muted = m;
            }
        }
    }

    #[inline]
    pub fn clock_quarter_frame(&mut self) {
        self.pulse_1.envelope.clock();
        self.pulse_2.envelope.clock();
        self.noise_generator.envelope.clock();
        self.triangle_generator.linear_counter.clock();
    }

    #[inline(always)]
    #[must_use]
    pub fn poll_irq(&self) -> bool { self.frame_counter.frame_interrupt }
}

#[derive(Debug, Clone, Default)]
pub struct DMCUnit {
    pub sample_end_irq_enable: bool,
    pub does_loop: bool,
    pub sample_rate: u8,
    pub load_counter: u8,
    pub sample_address: u8,
    pub sample_length: u8,
    pub bytes_remaining: u8,
}

#[derive(Debug, Clone, Default)]
pub struct NoiseGenerator {
    pub envelope: EnvelopeUnit,
    pub length_counter: LengthCounter,
    noise_period: u16,
    pub noise_mode: bool,
}

impl NoiseGenerator {
    pub fn set_period(&mut self, index: u8) { self.noise_period = NOISE_TABLE[index as usize]; }
}

#[derive(Debug, Clone, Default)]
pub struct PulseGenerator {
    pub envelope: EnvelopeUnit,
    pub length_counter: LengthCounter,
    pub sweep_unit: SweepUnit,
    pub period: u16,
    pub muted: bool,
    pub duty_ratio: u8,
    pub phase: u8,
}

#[derive(Debug, Clone, Default)]
pub struct TriangleGenerator {
    pub length_counter: LengthCounter,
    pub linear_counter: LinearCounter,
    pub period: u16,
}

#[derive(Debug, Clone, Default)]
pub struct SweepUnit {
    pub shift_bits: u8,
    pub negate: bool,
    pub counter_reload: u8,
    pub enable: bool,
    counter: u8,
    ones_complement: bool,
    pub reload: bool,
}

impl SweepUnit {
    pub fn sweep(&self, period: u16) -> u16 {
        let change = period >> self.shift_bits;
        let change = if self.negate {
            change as i16
        } else {
            0 - change as i16
        };

        let target = if self.ones_complement {
            period as i32 + change as i32 - 1
        } else {
            period as i32 + change as i32
        };

        if target < 0 { 0 } else { target as u16 }
    }

    pub fn clock(&mut self, period: u16) -> Result<u16, bool> {
        let target_period = self.sweep(period);
        if self.counter == 0 && self.enable && self.shift_bits != 0 {
            if target_period > 0x7FF {
                Err(true)
            } else {
                Ok(target_period)
            }
        } else if self.counter == 0 || self.reload {
            self.counter = self.counter_reload;
            self.reload = false;
            Err(false)
        } else {
            self.counter -= 1;
            Err(false)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LinearCounter {
    pub reload_flag: bool,
    pub control_flag: bool,
    counter: u8,
    pub reload_value: u8,
}

impl LinearCounter {
    pub fn clock(&mut self) {
        if self.reload_flag {
            self.counter = self.reload_value;
        } else if self.counter != 0 {
            self.counter -= 1;
        }

        if !self.control_flag {
            self.reload_flag = false;
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LengthCounter {
    pub halt: bool,
    enabled: bool,
    length_counter: u8,
}

impl LengthCounter {
    pub fn clock(&mut self) {
        if self.length_counter != 0 && !self.halt {
            self.length_counter -= 1;
        }
    }

    pub fn load(&mut self, index: u8) {
        if self.enabled {
            self.length_counter = LENGTH_COUNTER_TABLE[index as usize]
        } else {
            self.length_counter = 0;
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;

        if self.enabled {
            self.length_counter = 0;
        }
    }

    pub fn get_counter(&self) -> u8 { self.length_counter }
}

#[derive(Debug, Clone, Default)]
pub struct EnvelopeUnit {
    pub start: bool,
    pub divider: u8,
    pub decay_counter: u8,
    pub divider_reload: u8,
    pub does_loop: bool,
    pub const_volume: bool,
}

impl EnvelopeUnit {
    pub fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay_counter = 15;
            self.divider = self.divider_reload;
        } else if self.divider == 0 {
            self.divider = self.divider_reload;

            if self.decay_counter != 0 {
                self.decay_counter -= 1;
            } else if self.does_loop {
                self.decay_counter = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    pub fn get_output(&self) -> u8 {
        if self.const_volume {
            self.divider_reload
        } else {
            self.decay_counter
        }
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq, Default, Serialize, Deserialize)]
pub struct FrameCounter {
    pub external_sync: bool,
    pub frame_interrupt: bool,
    pub interrupt_inhibit: bool,
    pub apu_cycle_counter: u16,
}

impl FrameCounter {
    #[inline]
    pub fn clock(&mut self, is_proper: bool) -> Option<FrameCounterClockResult> {
        if is_proper {
            self.apu_cycle_counter += 1;

            match self.apu_cycle_counter {
                FRAME_COUNTER_STEP_1 | FRAME_COUNTER_STEP_3 => {
                    Some(FrameCounterClockResult::QuarterFrame)
                }
                FRAME_COUNTER_STEP_2 | FRAME_COUNTER_STEP_5 => {
                    Some(FrameCounterClockResult::HalfFrame)
                }
                FRAME_COUNTER_STEP_4 => {
                    if self.external_sync {
                        None
                    } else {
                        Some(FrameCounterClockResult::HalfFrame)
                    }
                }
                _ => None,
            }
        } else {
            if !self.external_sync && !self.interrupt_inhibit {
                if self.apu_cycle_counter == FRAME_COUNTER_STEP_4 - 1
                    || self.apu_cycle_counter == FRAME_COUNTER_STEP_4
                    || self.apu_cycle_counter == FRAME_COUNTER_STEP_4 + 1
                {
                    self.frame_interrupt = true;

                    if self.apu_cycle_counter == FRAME_COUNTER_STEP_4 {
                        self.apu_cycle_counter = 0;
                    }
                }
            } else if self.apu_cycle_counter == FRAME_COUNTER_STEP_5 {
                self.apu_cycle_counter = 0;
            }

            None
        }
    }

    #[inline(always)]
    pub fn get_frame_interrupt_for_register(&mut self) -> bool {
        let res = self.frame_interrupt;
        self.frame_interrupt = false;
        res
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FrameCounterClockResult {
    HalfFrame,
    QuarterFrame,
}
