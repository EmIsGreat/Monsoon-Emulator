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
    #[inline(always)]
    pub fn read(&self, addr: u32, _: &OpenBus) -> u8 {
        self.memory[addr as usize % self.memory.len()]
    }

    #[inline(always)]
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

    pub fn size(&self) -> usize { self.memory.len() }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct OpenBus {
    value: u8,
    timers: [u32; 8],
    decay_time: u32,
}

impl OpenBus {
    pub fn new(decay_time: u32) -> Self {
        Self {
            value: 0,
            timers: [decay_time; 8],
            decay_time,
        }
    }

    #[inline(always)]
    pub fn set_masked(&mut self, value: u8, mask: u8) {
        self.value = (self.value & !mask) | (value & mask);

        if mask != 0 {
            for bit in 0..8 {
                if mask & (1 << bit) != 0 {
                    self.timers[bit] = 0;
                }
            }
        }
    }

    #[inline(always)]
    pub fn tick(&mut self, times: u8) {
        let times = u32::from(times);
        let mut expired = 0u8;

        for i in 0..8 {
            let bit_timer = &mut self.timers[i];
            *bit_timer += times;

            if *bit_timer > self.decay_time {
                expired |= 1 << i;
                *bit_timer = 0;
            }
        }

        self.value &= !expired;
    }

    #[inline(always)]
    pub fn read(&self) -> u8 { self.value }
}

impl From<(&Vec<u8>, bool)> for Memory {
    fn from(value: (&Vec<u8>, bool)) -> Self {
        Memory {
            memory: value.0.clone().into_boxed_slice(),
            is_write: value.1,
        }
    }
}
