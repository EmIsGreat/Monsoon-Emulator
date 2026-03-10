use std::cell::Cell;

use crate::emulation::mem::MemoryDevice;

#[derive(Debug, Clone)]
pub struct ApuRegisters {
    input_a: u8,
    input_b: u8,
    shift_a: Cell<u8>,
    shift_b: Cell<u8>,
    strobe: bool,
}

impl Default for ApuRegisters {
    fn default() -> Self { Self::new() }
}

impl ApuRegisters {
    pub fn new() -> Self {
        Self {
            input_a: 0,
            input_b: 0,
            shift_a: 0.into(),
            shift_b: 0.into(),
            strobe: false,
        }
    }

    #[inline]
    fn poll_a(&self, open_bus: u8) -> u8 {
        let val = self.shift_a.get();
        let res = (open_bus & !0b111) | (val & 1);
        self.shift_a.replace((val >> 1) | 0x80);
        res
    }

    #[inline]
    fn poll_b(&self, open_bus: u8) -> u8 {
        let val = self.shift_b.get();
        let res = (open_bus & !0b111) | (val & 1);
        self.shift_b.replace((val >> 1) | 0x80);
        res
    }

    #[inline]
    fn strobe(&self) {
        if self.strobe {
            self.shift_a.replace(self.input_a);
            self.shift_b.replace(self.input_b);
        }
    }
}

impl MemoryDevice for ApuRegisters {
    #[inline]
    fn read(&self, addr: u16, open_bus: u8) -> u8 {
        self.strobe();

        match addr {
            0x00..=0x15 => open_bus,
            0x16 => self.poll_a(open_bus),
            0x17 => self.poll_b(open_bus),
            _ => 0,
        }
    }

    #[inline]
    fn write(&mut self, addr: u16, data: u8) {
        self.strobe();

        if addr == 0x16 {
            self.strobe = (data & 1) == 1;
        }
    }

    #[inline]
    fn init(&mut self, addr: u16, val: u8) {
        match addr {
            0x16 => self.input_a = val,
            0x17 => self.input_b = val,
            _ => {}
        }
    }

    #[inline]
    fn load(&mut self, _: Box<[u8]>) {}

    #[inline]
    fn is_internal(&self) -> bool { true }

    #[inline]
    fn snapshot(&self, addr: u16, open_bus: u8) -> u8 {
        match addr {
            0x00..=0x15 => open_bus,
            0x16 => self.poll_a(open_bus),
            0x17 => self.poll_b(open_bus),
            _ => 0,
        }
    }

    #[inline]
    fn snapshot_all(&self) -> Vec<u8> { vec![self.snapshot(0, 0)] }
}
