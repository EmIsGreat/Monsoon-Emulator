use std::fmt::Debug;
use std::hash::Hash;

use crate::emulation::rom::ExpansionDevice;

#[enum_delegate::implement(PeripheralDevice)]
pub enum Peripheral {
    StandardController(StandardController),
}

impl Default for Peripheral {
    fn default() -> Self { Peripheral::StandardController(StandardController::default()) }
}

#[enum_delegate::register]
pub trait PeripheralDevice {
    type InputType: Default;
    type RefreshVal: Default;
    fn set_refresh_func(&mut self, func: Box<dyn Fn() -> Self::InputType + Send + Sync + 'static>);
    fn refresh(&self) -> Self::RefreshVal;
    fn read(&mut self) -> u8;
    fn read_debug(&self) -> u8;
    fn handle_strobe_data(&mut self, data: u8);
}

impl From<ExpansionDevice> for Peripheral {
    fn from(value: ExpansionDevice) -> Self {
        #[allow(clippy::panic)]
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

#[derive(Default)]
pub struct StandardController {
    pub shift: u8,
    pub strobe: bool,
    refresh_func: Option<Box<dyn Fn() -> <Self as PeripheralDevice>::InputType + Send + Sync>>,
}

impl PeripheralDevice for StandardController {
    type InputType = StandardControllerState;
    type RefreshVal = u8;

    fn set_refresh_func(&mut self, func: Box<dyn Fn() -> Self::InputType + Send + Sync>) {
        self.refresh_func = Some(func);
    }

    fn refresh(&self) -> Self::RefreshVal {
        let input = self.refresh_func.as_ref().map(|f| f()).unwrap_or_default();

        u8::from(input.a)
            | u8::from(input.b) << 1
            | u8::from(input.select) << 2
            | u8::from(input.start) << 3
            | u8::from(input.up) << 4
            | u8::from(input.down) << 5
            | u8::from(input.left) << 6
            | u8::from(input.right) << 7
    }

    #[inline(always)]
    fn read(&mut self) -> u8 {
        if self.strobe {
            self.shift = self.refresh();
        }

        self.poll()
    }

    #[inline(always)]
    fn read_debug(&self) -> u8 {
        let mut shift = self.shift;

        if self.strobe {
            shift = self.refresh();
        }

        StandardController::poll_with_shift(shift)
    }

    #[inline]
    fn handle_strobe_data(&mut self, data: u8) {
        self.strobe = (data & 1) == 1;
        if self.strobe {
            self.shift = self.refresh();
        }
    }
}

impl StandardController {
    #[inline(always)]
    fn poll(&mut self) -> u8 {
        let res = self.shift & 1;
        self.shift = (self.shift >> 1) | 0x80;
        res
    }

    #[inline]
    fn poll_with_shift(shift: u8) -> u8 { shift & 1 }

    #[must_use]
    pub fn new(shift: u8, strobe: bool) -> Self {
        StandardController {
            shift,
            strobe,
            refresh_func: None,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub struct StandardControllerState {
    pub a: bool,
    pub b: bool,
    pub select: bool,
    pub start: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}
