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
    bits: [BitState; 8],
    decay_time: u32,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BitState {
    set: bool,
    decay_timer: u32,
}

impl OpenBus {
    pub fn new(decay_time: u32) -> Self {
        Self {
            bits: [BitState {
                set: false,
                decay_timer: 0,
            }; 8],
            decay_time,
        }
    }

    #[inline]
    pub fn set_masked(&mut self, value: u8, mask: u8) {
        for bit in 0..8 {
            let bit_mask = 1 << bit;
            if mask & bit_mask != 0 {
                let val = (value & bit_mask) != 0;
                self.bits[bit].set = val;
                self.bits[bit].decay_timer = 0;
            }
        }
    }

    #[inline]
    pub fn tick(&mut self, times: u8) {
        let times = times as u32;
        for bit in &mut self.bits {
            bit.decay_timer += times;
            if bit.decay_timer > self.decay_time {
                bit.set = false;
                bit.decay_timer = 0
            }
        }
    }

    #[inline]
    pub fn read(&self) -> u8 {
        self.bits
            .iter()
            .enumerate()
            .fold(0u8, |acc, (i, b)| acc | ((b.set as u8) << i))
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
