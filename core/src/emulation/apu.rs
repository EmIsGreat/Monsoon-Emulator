use serde::{Deserialize, Serialize};

pub const FRAME_COUNTER_STEP_1: u16 = 3728;
pub const FRAME_COUNTER_STEP_2: u16 = 7456;
pub const FRAME_COUNTER_STEP_3: u16 = 11185;
pub const FRAME_COUNTER_STEP_4: u16 = 14914;
pub const FRAME_COUNTER_STEP_5: u16 = 18640;

#[derive(Debug, Clone)]
pub struct Apu {
    pub frame_counter: FrameCounter,
}

impl Apu {
    #[inline(always)]
    pub fn clock_frame_counter(&mut self, is_proper: bool) {
        match self.frame_counter.clock(is_proper) {
            Some(res) => match res {
                FrameCounterClockResult::HalfFrame => self.clock_half_frame(),
                FrameCounterClockResult::QuarterFrame => self.clock_quarter_frame(),
            },
            None => {}
        }
    }

    #[inline]
    pub fn clock_half_frame(&mut self) { self.clock_quarter_frame(); }

    #[inline]
    pub fn clock_quarter_frame(&mut self) {}

    #[inline]
    pub fn poll_irq(&self) -> bool { self.frame_counter.frame_interrupt }
}

impl Default for Apu {
    fn default() -> Self {
        Self {
            frame_counter: FrameCounter::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PulseGenerator {}

impl Default for PulseGenerator {
    fn default() -> Self { Self {} }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct FrameCounter {
    pub five_step: bool,
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
                FRAME_COUNTER_STEP_1 => Some(FrameCounterClockResult::QuarterFrame),
                FRAME_COUNTER_STEP_2 => Some(FrameCounterClockResult::HalfFrame),
                FRAME_COUNTER_STEP_3 => Some(FrameCounterClockResult::QuarterFrame),
                FRAME_COUNTER_STEP_4 => {
                    if !self.five_step {
                        Some(FrameCounterClockResult::HalfFrame)
                    } else {
                        None
                    }
                }
                FRAME_COUNTER_STEP_5 => Some(FrameCounterClockResult::HalfFrame),
                _ => None,
            }
        } else {
            if !self.five_step {
                if self.apu_cycle_counter == FRAME_COUNTER_STEP_4 - 1
                    || self.apu_cycle_counter == FRAME_COUNTER_STEP_4
                {
                    self.frame_interrupt = !self.interrupt_inhibit;

                    if self.apu_cycle_counter == FRAME_COUNTER_STEP_4 {
                        self.apu_cycle_counter = 0
                    }
                }
            } else {
                if self.apu_cycle_counter == FRAME_COUNTER_STEP_5 {
                    self.apu_cycle_counter = 0;
                }
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

impl Default for FrameCounter {
    fn default() -> Self {
        Self {
            frame_interrupt: false,
            interrupt_inhibit: false,
            five_step: false,
            apu_cycle_counter: 0,
        }
    }
}
