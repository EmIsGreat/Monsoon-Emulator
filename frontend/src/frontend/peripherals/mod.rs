use egui::{InputState, Key, Modifiers};
use monsoon_core::emulation::peripherals::StandardControllerState;
use serde::{Deserialize, Serialize};

use crate::frontend::egui::keybindings::{
    BindVariant, Binding, HotkeyBinding, ModifierKey, OnKeyAction,
};

pub trait FromEguiInput {
    fn from_egui(input: &egui::InputState) -> Self;
}

impl FromEguiInput for StandardControllerState {
    fn from_egui(input: &InputState) -> Self {}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
