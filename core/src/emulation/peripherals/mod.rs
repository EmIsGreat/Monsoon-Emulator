use std::fmt::Debug;
use std::hash::Hash;

use serde::{Deserialize, Serialize};

use crate::emulation::rom::ExpansionDevice;

#[enum_delegate::implement(PeripheralDevice)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Peripheral {
    StandardController(StandardController),
}

impl Default for Peripheral {
    fn default() -> Self { Peripheral::StandardController(StandardController::default()) }
}

#[enum_delegate::register]
pub trait PeripheralDevice {
    fn read(&mut self) -> u8;
    fn read_debug(&self) -> u8;
    fn handle_strobe_data(&mut self, data: u8);
}

impl From<ExpansionDevice> for Peripheral {
    fn from(value: ExpansionDevice) -> Self {
        match value {
            ExpansionDevice::StandardController => {
                Peripheral::StandardController(StandardController::default())
            }
            ExpansionDevice::Unknown(id) => {
                panic!("Peripheral with id \"{id}\" is not known")
            }
            _ => {
                unreachable!()
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Default, Serialize, Deserialize)]
pub struct StandardController {
    pub input: u8,
    shift: u8,
    strobe: bool,
}

impl PeripheralDevice for StandardController {
    #[inline]
    fn read(&mut self) -> u8 {
        if self.strobe {
            self.shift = self.input
        }

        self.poll()
    }

    #[inline]
    fn read_debug(&self) -> u8 {
        let mut shift = self.shift;

        if self.strobe {
            shift = self.input;
        }

        self.poll_with_shift(shift)
    }

    #[inline]
    fn handle_strobe_data(&mut self, data: u8) {
        self.strobe = (data & 1) == 1;
        if self.strobe {
            self.shift = self.input
        }
    }
}

impl StandardController {
    #[inline]
    fn poll(&mut self) -> u8 {
        let res = self.shift & 1;
        self.shift = (self.shift >> 1) | 0x80;
        res
    }

    #[inline]
    fn poll_with_shift(&self, shift: u8) -> u8 { shift & 1 }
}
