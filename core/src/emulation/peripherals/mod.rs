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
    fn set_state(&mut self, state: ControllerState);
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
    state: ControllerState,
}

impl Clone for StandardController {
    fn clone(&self) -> Self {
        Self {
            shift: self.shift,
            strobe: self.strobe,
            state: ControllerState::default(),
        }
    }
}

impl PeripheralDevice for StandardController {
    fn set_state(&mut self, state: ControllerState) { self.state = state; }

    #[inline(always)]
    fn read(&mut self) -> u8 {
        if self.strobe {
            self.refresh();
        }

        self.poll()
    }

    #[inline(always)]
    fn read_debug(&self) -> u8 {
        let mut cloned = (*self).clone();

        if cloned.strobe {
            cloned.refresh();
        }

        cloned.read()
    }

    #[inline]
    fn handle_strobe_data(&mut self, data: u8) {
        self.strobe = (data & 1) == 1;

        if self.strobe {
            self.refresh();
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

    #[must_use]
    pub fn new(shift: u8, strobe: bool) -> Self {
        StandardController {
            shift,
            strobe,
            state: ControllerState::default(),
        }
    }

    fn refresh(&mut self) {
        if let Some(input) = self.state.standard_controller {
            self.shift = u8::from(input.a)
                | u8::from(input.b) << 1
                | u8::from(input.select) << 2
                | u8::from(input.start) << 3
                | u8::from(input.up) << 4
                | u8::from(input.down) << 5
                | u8::from(input.left) << 6
                | u8::from(input.right) << 7;
        } else {
            self.shift = 0;
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

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub struct ControllerState {
    standard_controller: Option<StandardControllerState>,
}

impl ControllerState {
    #[must_use]
    pub fn with_standard_controller_state(mut self, state: StandardControllerState) -> Self {
        self.standard_controller = Some(state);
        self
    }
}
