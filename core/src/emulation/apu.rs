use rtrb::Producer;
use serde::{Deserialize, Serialize};

pub const APU_FREQUENCY: u32 = 894_886;
pub const SAMPLE_RATE: u32 = 48_000;
const FRAME_COUNTER_STEP_1: u16 = 3728;
const FRAME_COUNTER_STEP_2: u16 = 7456;
const FRAME_COUNTER_STEP_3: u16 = 11185;
const FRAME_COUNTER_STEP_4: u16 = 14914;
const FRAME_COUNTER_STEP_5: u16 = 18640;
const ONE_OVER_8227: f32 = 1.0 / 8227.0;
const ONE_OVER_12241: f32 = 1.0 / 12241.0;
const ONE_OVER_22638: f32 = 1.0 / 22638.0;
const LENGTH_COUNTER_TABLE: [u8; 0x20] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

const NOISE_TABLE: [u16; 0x10] = [
    2, 4, 8, 16, 32, 48, 64, 80, 101, 127, 190, 254, 391, 508, 1017, 2034,
];

const TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

#[derive(Debug)]
pub struct Apu {
    pub frame_counter: FrameCounter,
    pub pulse_1: PulseGenerator,
    pub pulse_2: PulseGenerator,
    pub noise_generator: NoiseGenerator,
    pub triangle_generator: TriangleGenerator,
    pub dmc_unit: DMCUnit,
    pub sample_producer: Option<Producer<f32>>,
    pub sample_phase: u32,
}

impl Clone for Apu {
    fn clone(&self) -> Self {
        Self {
            frame_counter: self.frame_counter.clone(),
            pulse_1: self.pulse_1.clone(),
            pulse_2: self.pulse_2.clone(),
            noise_generator: self.noise_generator.clone(),
            triangle_generator: self.triangle_generator.clone(),
            dmc_unit: self.dmc_unit.clone(),
            sample_producer: None,
            sample_phase: 0,
        }
    }
}

impl Default for Apu {
    fn default() -> Self {
        let mut pulse_1 = PulseGenerator::default();
        pulse_1.sweep_unit.ones_complement = true;

        Self {
            frame_counter: FrameCounter::default(),
            pulse_1,
            pulse_2: PulseGenerator::default(),
            noise_generator: NoiseGenerator::default(),
            triangle_generator: TriangleGenerator::default(),
            dmc_unit: DMCUnit::default(),
            sample_producer: None,
            sample_phase: 0,
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

        let sample = self.mix();

        if let Some(producer) = &mut self.sample_producer {
            self.sample_phase += SAMPLE_RATE;

            if self.sample_phase >= APU_FREQUENCY {
                self.sample_phase -= APU_FREQUENCY;

                let _ = producer.push(sample);
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

    pub fn mix(&mut self) -> f32 {
        let pulse_1 = self.pulse_1.clock();
        let pulse_2 = self.pulse_2.clock();

        let pulse_sum = f32::from(pulse_1 + pulse_2);
        let pulse_out = if pulse_sum == 0.0 {
            0.0
        } else {
            let pulse_div = (8128.0 / pulse_sum) + 100.0;
            95.88 / pulse_div
        };

        let triangle = self.triangle_generator.clock();
        let noise = self.noise_generator.clock();
        let dmc = self.dmc_unit.clock();

        let triangle_div = f32::from(triangle) * ONE_OVER_8227;
        let noise_div = f32::from(noise) * ONE_OVER_12241;
        let dmc_div = f32::from(dmc) * ONE_OVER_22638;

        let tnc_sum = triangle_div + noise_div + dmc_div;

        let tnc_out = if tnc_sum == 0.0 {
            0.0
        } else {
            let tnc_div = 1.0 / tnc_sum;
            let tnc_added = tnc_div + 100.0;
            159.79 / tnc_added
        };

        tnc_out + pulse_out
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct DMCUnit {
    pub irq_asserted: bool,
    pub irq_pending: bool,
    pub sample_end_irq_enable: bool,
    pub does_loop: bool,
    pub sample_rate: u8,
    pub load_counter: u8,
    pub sample_address: u8,
    pub sample_length: u8,
    pub bytes_remaining: u8,
}

impl DMCUnit {
    pub fn restart_sample_playback(&mut self) {
        // TODO
    }

    pub fn clock(&mut self) -> u8 { 0 }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct NoiseGenerator {
    pub envelope: EnvelopeUnit,
    pub length_counter: LengthCounter,
    noise_period: u16,
    pub noise_mode: bool,
}

impl NoiseGenerator {
    pub fn set_period(&mut self, index: u8) { self.noise_period = NOISE_TABLE[index as usize]; }

    pub fn clock(&mut self) -> u8 { 0 }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct PulseGenerator {
    pub envelope: EnvelopeUnit,
    pub length_counter: LengthCounter,
    pub sweep_unit: SweepUnit,
    pub period: u16,
    pub muted: bool,
    pub duty_ratio: u8,
    pub phase: u8,
}

impl PulseGenerator {
    pub fn clock(&mut self) -> u8 { 0 }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct TriangleGenerator {
    pub length_counter: LengthCounter,
    pub linear_counter: LinearCounter,
    pub period: u16,
    pub counter: u16,
    pub waveform_sequence_index: u8,
}

impl TriangleGenerator {
    pub fn clock(&mut self) -> u8 {
        let counter = self.counter.saturating_sub(2);
        self.counter = counter;

        if self.counter == 0 && self.linear_counter.counter != 0 && self.length_counter.counter != 0
        {
            self.counter = self.period;
            self.waveform_sequence_index += 1;
        }

        TRIANGLE_TABLE[self.waveform_sequence_index as usize]
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[allow(clippy::struct_excessive_bools)]
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
    #[must_use]
    pub fn sweep(&self, period: u16) -> u16 {
        let change = period >> self.shift_bits;
        let change = if self.negate {
            change.cast_signed()
        } else {
            0 - change.cast_signed()
        };

        let target = if self.ones_complement {
            i32::from(period) + i32::from(change) - 1
        } else {
            i32::from(period) + i32::from(change)
        };

        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_possible_truncation)]
        if target < 0 { 0 } else { target as u16 }
    }

    #[allow(clippy::missing_errors_doc)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct LinearCounter {
    pub reload_flag: bool,
    pub control_flag: bool,
    counter: u8,
    pub reload_value: u8,
}

impl LinearCounter {
    pub fn clock(&mut self) {
        if self.reload_flag {
            println!("{}", self.reload_value);
            self.counter = self.reload_value;
        } else if self.counter != 0 {
            self.counter -= 1;
        }

        if !self.control_flag {
            self.reload_flag = false;
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct LengthCounter {
    pub halt: bool,
    enabled: bool,
    counter: u8,
}

impl LengthCounter {
    pub fn clock(&mut self) {
        if self.counter != 0 && !self.halt {
            self.counter -= 1;
        }
    }

    pub fn load(&mut self, index: u8) {
        if self.enabled {
            self.counter = LENGTH_COUNTER_TABLE[index as usize];
        } else {
            self.counter = 0;
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;

        if self.enabled {
            self.counter = 0;
        }
    }

    #[must_use]
    pub fn get_counter(&self) -> u8 { self.counter }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Default)]
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

    #[must_use]
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
            self.apu_cycle_counter = self.apu_cycle_counter.wrapping_add(1);

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
