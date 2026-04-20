use std::fmt::Debug;

use serde::{Deserialize, Serialize};

pub mod palette_ram;

pub trait MemoryDevice: Debug {
    fn read(&self, addr: u16, open_bus: &OpenBus) -> u8;
    fn write(&mut self, addr: u16, data: u8);
    fn init(&mut self, addr: u16, data: u8);
    fn load(&mut self, data: Box<[u8]>);

    fn is_internal(&self) -> bool { false }
    fn snapshot(&self, addr: u16, open_bus: &OpenBus) -> u8 { self.read(addr, open_bus) }
    fn snapshot_all(&self) -> Vec<u8>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Ram {
    memory: Box<[u8]>,
}

impl Ram {
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "RAM size must be greater than zero");

        Self {
            memory: vec![0; size].into_boxed_slice(),
        }
    }
}

impl MemoryDevice for Ram {
    #[inline]
    fn read(&self, addr: u16, _: &OpenBus) -> u8 {
        self.memory[addr as usize % self.memory.len()]
    }

    #[inline]
    fn write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize % self.memory.len()] = data;
    }

    #[inline]
    fn init(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize % self.memory.len()] = data;
    }

    fn load(&mut self, data: Box<[u8]>) { self.memory = data }

    fn snapshot_all(&self) -> Vec<u8> { self.memory.to_vec() }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Rom {
    memory: Box<[u8]>,
}

impl Rom {
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "ROM size must be greater than zero");

        Self {
            memory: vec![0; size].into_boxed_slice(),
        }
    }
}

impl MemoryDevice for Rom {
    #[inline]
    fn read(&self, addr: u16, _: &OpenBus) -> u8 {
        self.memory[addr as usize % self.memory.len()]
    }

    #[inline]
    fn write(&mut self, _: u16, _: u8) {}

    #[inline]
    fn init(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize % self.memory.len()] = data;
    }

    fn load(&mut self, data: Box<[u8]>) { self.memory = data }

    fn snapshot_all(&self) -> Vec<u8> { self.memory.to_vec() }
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
        let times_u32 = times as u32;
        for bit in &mut self.bits {
            bit.decay_timer += times_u32;
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

impl From<&Vec<u8>> for Ram {
    fn from(value: &Vec<u8>) -> Self {
        Ram {
            memory: value.clone().into_boxed_slice(),
        }
    }
}

impl From<&Vec<u8>> for Rom {
    fn from(value: &Vec<u8>) -> Self {
        Rom {
            memory: value.clone().into_boxed_slice(),
        }
    }
}
