use std::fmt::Debug;

use serde::{Deserialize, Serialize};

pub mod palette_ram;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Memory {
    memory: Box<[u8]>,
    pub is_write: bool,
}

impl Memory {
    pub fn new(size: usize, is_write: bool) -> Self {
        assert!(size > 0, "RAM size must be greater than zero");

        Self {
            memory: vec![0; size].into_boxed_slice(),
            is_write,
        }
    }
}

impl Memory {
    #[inline]
    pub fn read(&self, addr: u32, _: &OpenBus) -> u8 {
        self.memory[addr as usize % self.memory.len()]
    }

    #[inline]
    pub fn write(&mut self, addr: u32, data: u8) {
        if !self.is_write {
            return;
        }

        self.memory[addr as usize % self.memory.len()] = data;
    }

    #[inline]
    pub fn init(&mut self, addr: u32, data: u8) {
        self.memory[addr as usize % self.memory.len()] = data;
    }

    pub fn load(&mut self, data: Box<[u8]>) { self.memory = data }

    pub fn snapshot(&self, addr: u32, open_bus: &OpenBus) -> u8 { self.read(addr, open_bus) }

    pub fn snapshot_all(&self) -> Vec<u8> { self.memory.to_vec() }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct OpenBus {
    bits: [bool; 8],
    timers: [u32; 8],
    decay_time: u32,
}

impl OpenBus {
    pub fn new(decay_time: u32) -> Self {
        Self {
            bits: [false; 8],
            timers: [decay_time; 8],
            decay_time,
        }
    }

    #[inline]
    pub fn set_masked(&mut self, value: u8, mask: u8) {
        for bit in 0..8 {
            let bit_mask = 1 << bit;
            if mask & bit_mask != 0 {
                let val = (value & bit_mask) != 0;
                self.bits[bit] = val;
                self.timers[bit] = 0;
            }
        }
    }

    #[inline]
    pub fn tick(&mut self, times: u8) {
        let times = times as u32;
        for (i, bit) in &mut self.bits.iter_mut().enumerate() {
            self.timers[i] += times;
            if self.timers[i] > self.decay_time {
                *bit = false;
                self.timers[i] = 0
            }
        }
    }

    #[inline]
    pub fn read(&self) -> u8 { Self::bools_to_u8(self.bits) }

    #[inline(always)]
    fn bools_to_u8(bits: [bool; 8]) -> u8 {
        (bits[0] as u8) << 0
            | (bits[1] as u8) << 1
            | (bits[2] as u8) << 2
            | (bits[3] as u8) << 3
            | (bits[4] as u8) << 4
            | (bits[5] as u8) << 5
            | (bits[6] as u8) << 6
            | (bits[7] as u8) << 7
    }
}

impl From<(&Vec<u8>, bool)> for Memory {
    fn from(value: (&Vec<u8>, bool)) -> Self {
        Memory {
            memory: value.0.clone().into_boxed_slice(),
            is_write: value.1,
        }
    }
}
