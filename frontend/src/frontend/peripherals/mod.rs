use egui::{InputState, Key, Modifiers};
use monsoon_core::emulation::peripherals::StandardControllerState;
use serde::{Deserialize, Serialize};

use crate::frontend::egui::keybindings::{
    BindVariant, Binding, HotkeyBinding, ModifierKey, OnKeyAction,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StandardControllerBindings {
    pub up: Binding,
    pub down: Binding,
    pub left: Binding,
    pub right: Binding,
    pub a: Binding,
    pub b: Binding,
    pub start: Binding,
    pub select: Binding,
}

impl Default for StandardControllerBindings {
    fn default() -> Self {
        Self {
            up: Binding::key(Key::W, OnKeyAction::StdControllerUp),
            down: Binding::key(Key::S, OnKeyAction::StdControllerDown),
            left: Binding::key(Key::A, OnKeyAction::StdControllerLeft),
            right: Binding::key(Key::D, OnKeyAction::StdControllerRight),

            a: Binding::key(Key::Space, OnKeyAction::StdControllerAButton),
            b: Binding::new(
                BindVariant::ModifierKey(ModifierKey::Shift),
                Modifiers::NONE,
                OnKeyAction::StdControllerBButton,
            ),

            start: Binding::key(Key::Enter, OnKeyAction::StdControllerStartButton),
            select: Binding::key(Key::Tab, OnKeyAction::StdControllerSelectButton),
        }
    }
}

impl StandardControllerBindings {
    #[must_use]
    pub fn to_state(&self, input: &InputState) -> StandardControllerState {
        StandardControllerState {
            a: self.a.active(input),
            b: self.b.active(input),
            select: self.select.active(input),
            start: self.start.active(input),
            up: self.up.active(input),
            down: self.down.active(input),
            left: self.left.active(input),
            right: self.right.active(input),
        }
    }
}
