use serde::{Deserialize, Serialize};

pub const FRAME_COUNTER_STEP_1: u16 = 3728;
pub const FRAME_COUNTER_STEP_2: u16 = 7456;
pub const FRAME_COUNTER_STEP_3: u16 = 11185;
pub const FRAME_COUNTER_STEP_4: u16 = 14914;
pub const FRAME_COUNTER_STEP_5: u16 = 18640;

#[derive(Debug, Clone, Default)]
pub struct Apu {
    pub frame_counter: FrameCounter,
}

impl Apu {
    #[inline]
    pub fn clock_frame_counter(&mut self, is_proper: bool) {
        if let Some(res) = self.frame_counter.clock(is_proper) {
            match res {
                FrameCounterClockResult::HalfFrame => self.clock_half_frame(),
                FrameCounterClockResult::QuarterFrame => self.clock_quarter_frame(),
            }
        }
    }

    #[inline]
    pub fn clock_half_frame(&mut self) { self.clock_quarter_frame(); }

    #[inline]
    pub fn clock_quarter_frame(&mut self) {}

    #[inline]
    #[must_use]
    pub fn poll_irq(&self) -> bool { self.frame_counter.frame_interrupt }
}

#[derive(Debug, Clone, Default)]
pub struct PulseGenerator {}

#[derive(Debug, Clone, Eq, Hash, PartialEq, Default, Serialize, Deserialize)]
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
                FRAME_COUNTER_STEP_1 | FRAME_COUNTER_STEP_3 => {
                    Some(FrameCounterClockResult::QuarterFrame)
                }
                FRAME_COUNTER_STEP_2 | FRAME_COUNTER_STEP_5 => {
                    Some(FrameCounterClockResult::HalfFrame)
                }
                FRAME_COUNTER_STEP_4 => {
                    if self.five_step {
                        None
                    } else {
                        Some(FrameCounterClockResult::HalfFrame)
                    }
                }
                _ => None,
            }
        } else {
            if !self.five_step {
                if self.apu_cycle_counter == FRAME_COUNTER_STEP_4 - 1
                    || self.apu_cycle_counter == FRAME_COUNTER_STEP_4
                {
                    self.frame_interrupt = !self.interrupt_inhibit;

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

    #[inline]
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
