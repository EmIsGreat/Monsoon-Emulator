//! Keybindings configuration for the emulator.
//!
//! This module provides configurable keybindings for:
//! - NES controller inputs (Up, Down, Left, Right, A, B, Start, Select)
//! - Emulation controls (Pause, Reset, Quicksave, Quickload)
//! - Debug controls (Cycle palette)
//!
//! This module includes a port of the egui_hotkey crate's functionality
//! updated to work with egui 0.33.

use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::hash::Hash;

use crossbeam_channel::Sender;
use egui::{
    Event, Id, InputState, Key, Modifiers, PointerButton, Response, Sense, Ui, Widget, vec2,
};
use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::frontend::messages::AsyncFrontendMessage;
use crate::messages::ControllerEvent;

/// Well-known egui data ID used to signal that a [`Hotkey`] widget is
/// currently waiting for the user to press a key.
///
/// The input handler checks this state so it can skip normal key handling while
/// the keybinding UI is active.
fn hotkey_active_id() -> Id { Id::new("hotkey_active_widget_id") }
fn hotkey_just_set_this_frame_id() -> Id { Id::new("hotkey_just_set_this_frame") }
fn hotkey_suppressed_binding_id() -> Id { Id::new("hotkey_suppressed_binding_until_release") }

#[derive(Clone, Copy)]
pub(crate) struct SuppressedBinding {
    pub action: OnKeyAction,
    pub binding: Binding,
}

/// Returns true if any hotkey widget is currently waiting for key input.
pub(crate) fn hotkey_is_any_expecting(ctx: &egui::Context) -> bool {
    ctx.memory_mut(|memory| memory.data.get_temp::<Id>(hotkey_active_id()).is_some())
}

/// Returns whether a hotkey was captured this frame and clears the flag.
pub(crate) fn hotkey_take_just_set_this_frame(ctx: &egui::Context) -> bool {
    ctx.memory_mut(|memory| {
        if memory
            .data
            .get_temp::<bool>(hotkey_just_set_this_frame_id())
            .unwrap_or(false)
        {
            memory.data.remove::<bool>(hotkey_just_set_this_frame_id());
            true
        } else {
            false
        }
    })
}

pub(crate) fn hotkey_get_suppressed_binding(ctx: &egui::Context) -> Option<SuppressedBinding> {
    ctx.memory_mut(|memory| {
        memory
            .data
            .get_temp::<SuppressedBinding>(hotkey_suppressed_binding_id())
    })
}

pub(crate) fn hotkey_set_suppressed_binding(
    ctx: &egui::Context,
    suppressed: Option<SuppressedBinding>,
) {
    ctx.memory_mut(|memory| {
        if let Some(suppressed) = suppressed {
            memory
                .data
                .insert_temp(hotkey_suppressed_binding_id(), suppressed);
        } else {
            memory
                .data
                .remove::<SuppressedBinding>(hotkey_suppressed_binding_id());
        }
    });
}

// ============================================================================
// Binding types (ported from egui_hotkey)
// ============================================================================

/// A standalone modifier key (Shift, Ctrl, Alt, Command, or MacCmd) used as a
/// binding target.
///
/// Unlike regular [`Key`] variants, these modifier keys are not part of egui's
/// `Key` enum and are only tracked via [`Modifiers`].  This variant allows
/// users to bind a bare modifier key to a controller button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierKey {
    Shift,
    Ctrl,
    Alt,
    Command,
    MacCmd,
}

impl ModifierKey {
    /// Returns `true` if this modifier key is currently held down.
    pub fn is_down(&self, input: &InputState) -> bool {
        match self {
            ModifierKey::Shift => input.modifiers.shift,
            ModifierKey::Ctrl => input.modifiers.ctrl,
            ModifierKey::Alt => input.modifiers.alt,
            ModifierKey::Command => input.modifiers.command,
            ModifierKey::MacCmd => input.modifiers.mac_cmd,
        }
    }
}

impl Display for ModifierKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ModifierKey::Shift => f.write_str("Shift"),
            ModifierKey::Ctrl => f.write_str("Ctrl"),
            ModifierKey::Alt => f.write_str("Alt"),
            ModifierKey::Command => f.write_str("Command"),
            ModifierKey::MacCmd => f.write_str("Command"),
        }
    }
}

/// Variant for binding. This can be a [`PointerButton`], a [`Key`], or a
/// standalone [`ModifierKey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindVariant {
    /// Represents an intentionally cleared binding.
    Unbound,
    Mouse(PointerButton),
    Keyboard(Key),
    /// A bare modifier key (e.g., Shift / Ctrl / Alt / Command / MacCmd) used
    /// on its own.
    ModifierKey(ModifierKey),
}

impl BindVariant {
    /// Returns true if the variant was pressed.
    ///
    /// Note: [`ModifierKey`] bindings always return `false` here because egui
    /// does not emit single-frame press events for modifier keys.  Use
    /// [`down`](Self::down) for continuous (held) checks instead, which is
    /// what controller inputs use.
    pub fn pressed(&self, input_state: &InputState) -> bool {
        match self {
            BindVariant::Unbound => false,
            BindVariant::Mouse(mb) => input_state.events.iter().any(|e| {
                matches!(e, Event::PointerButton {
                    button,
                    pressed: true,
                    ..
                } if mb == button)
            }),
            BindVariant::Keyboard(kb) => input_state.key_pressed(*kb),
            // egui doesn't emit Event::Key for modifier-only presses, so we
            // cannot distinguish "just pressed" from "still held".
            BindVariant::ModifierKey(_) => false,
        }
    }

    /// Returns true if the variant is down.
    pub fn down(&self, input_state: &InputState) -> bool {
        match self {
            BindVariant::Unbound => false,
            BindVariant::Mouse(mb) => input_state.pointer.button_down(*mb),
            BindVariant::Keyboard(kb) => input_state.key_down(*kb),
            BindVariant::ModifierKey(mk) => mk.is_down(input_state),
        }
    }
}

impl From<PointerButton> for BindVariant {
    fn from(pb: PointerButton) -> Self { Self::Mouse(pb) }
}

impl From<Key> for BindVariant {
    fn from(key: Key) -> Self { Self::Keyboard(key) }
}

impl Display for BindVariant {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            BindVariant::Unbound => f.write_str("None"),
            BindVariant::Mouse(pb) => f.write_str(match pb {
                PointerButton::Primary => "M1",
                PointerButton::Secondary => "M2",
                PointerButton::Middle => "M3",
                PointerButton::Extra1 => "M4",
                PointerButton::Extra2 => "M5",
            }),
            BindVariant::Keyboard(kb) => write!(f, "{}", kb.name()),
            BindVariant::ModifierKey(mk) => write!(f, "{mk}"),
        }
    }
}

type HotKeyCallback = dyn Fn(&Sender<AsyncFrontendMessage>);

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash, Ord, PartialOrd, EnumIter,
)]
pub enum OnKeyAction {
    ControllerUp,
    ControllerDown,
    ControllerLeft,
    ControllerRight,
    ControllerAButton,
    ControllerBButton,
    ControllerStartButton,
    ControllerSelectButton,
    PauseEmulator,
    StepFrame,
    StepScanline,
    StepMasterCycle,
    StepPpuCycle,
    StepCpuCycle,
    Reset,
    Quicksave,
    Quickload,
    ChangeDebugPalette,
    LoadRom,
    Quit,
    LoadSavestate,
    CreateSavestate,
    BrowseSavestates,
    PowerCycle,
    PowerToggle,
    OpenOptionsMenu,
    OpenKeybindingsMenu,
    OpenPaletteViewer,
    OpenPatternTableViewer,
    OpenNametableViewer,
    OpenSpriteViewer,
    OpenSoamViewer,
    OpenRomHeaderViewer,
    Speedup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerType {
    Single,
    Continuous,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, EnumIter, Hash, Ord, PartialOrd,
)]
pub enum KeybindCategory {
    Controller,
    Debug,
    Ui,
    Console,
}

impl KeybindCategory {
    pub fn get_name(&self) -> &'static str {
        match self {
            KeybindCategory::Controller => "Controller Keybinds",
            KeybindCategory::Debug => "Debug Keybinds",
            KeybindCategory::Ui => "Ui Shortcuts",
            KeybindCategory::Console => "Console Keybinds",
        }
    }
}

impl OnKeyAction {
    pub fn get_display_name(&self) -> &'static str {
        match self {
            OnKeyAction::ChangeDebugPalette => "Change Debug Palette",
            OnKeyAction::ControllerUp => "DPad Up",
            OnKeyAction::ControllerDown => "DPad Down",
            OnKeyAction::ControllerLeft => "DPad Left",
            OnKeyAction::ControllerRight => "DPad Right",
            OnKeyAction::ControllerAButton => "A Button",
            OnKeyAction::ControllerBButton => "B Button",
            OnKeyAction::ControllerStartButton => "Start Button",
            OnKeyAction::ControllerSelectButton => "Select Button",
            OnKeyAction::PauseEmulator => "Pause/Resume Emulator",
            OnKeyAction::StepFrame => "Step Frame",
            OnKeyAction::StepScanline => "Step Scanline",
            OnKeyAction::StepMasterCycle => "Step Master Cycle",
            OnKeyAction::StepPpuCycle => "Step Ppu Cycle",
            OnKeyAction::StepCpuCycle => "Step Cpu Cycle",
            OnKeyAction::Reset => "Reset Console",
            OnKeyAction::Quicksave => "Quicksave",
            OnKeyAction::Quickload => "Quickload",
            OnKeyAction::LoadRom => "Load Rom",
            OnKeyAction::Quit => "Quit",
            OnKeyAction::LoadSavestate => "Load Savestate",
            OnKeyAction::CreateSavestate => "Create Savestate",
            OnKeyAction::BrowseSavestates => "Browse Savestates",
            OnKeyAction::PowerCycle => "Power Cycle",
            OnKeyAction::PowerToggle => "Toggle Power",
            OnKeyAction::Speedup => "Speedup",
            OnKeyAction::OpenOptionsMenu => "Open Options",
            OnKeyAction::OpenKeybindingsMenu => "Open Keybinds",
            OnKeyAction::OpenPaletteViewer => "Open Palette Viewer",
            OnKeyAction::OpenPatternTableViewer => "Open Pattern Table Viewer",
            OnKeyAction::OpenNametableViewer => "Open Nametable Viewer",
            OnKeyAction::OpenSpriteViewer => "Open Sprite Viewer",
            OnKeyAction::OpenSoamViewer => "Open SOAM Viewer",
            OnKeyAction::OpenRomHeaderViewer => "Open ROM Header Viewer",
        }
    }

    pub fn get_trigger_type(&self) -> TriggerType {
        match self {
            OnKeyAction::ControllerUp
            | OnKeyAction::ControllerDown
            | OnKeyAction::ControllerLeft
            | OnKeyAction::ControllerRight
            | OnKeyAction::ControllerAButton
            | OnKeyAction::ControllerBButton
            | OnKeyAction::ControllerStartButton
            | OnKeyAction::ControllerSelectButton
            | OnKeyAction::Reset
            | OnKeyAction::Speedup => TriggerType::Continuous,
            _ => TriggerType::Single,
        }
    }

    pub fn get_category(&self) -> KeybindCategory {
        match self {
            OnKeyAction::ControllerUp
            | OnKeyAction::ControllerDown
            | OnKeyAction::ControllerLeft
            | OnKeyAction::ControllerRight
            | OnKeyAction::ControllerAButton
            | OnKeyAction::ControllerBButton
            | OnKeyAction::ControllerStartButton
            | OnKeyAction::ControllerSelectButton => KeybindCategory::Controller,
            OnKeyAction::PauseEmulator
            | OnKeyAction::StepFrame
            | OnKeyAction::StepScanline
            | OnKeyAction::StepMasterCycle
            | OnKeyAction::StepPpuCycle
            | OnKeyAction::StepCpuCycle
            | OnKeyAction::Quicksave
            | OnKeyAction::Quickload
            | OnKeyAction::OpenNametableViewer
            | OnKeyAction::OpenPaletteViewer
            | OnKeyAction::OpenPatternTableViewer
            | OnKeyAction::OpenSpriteViewer
            | OnKeyAction::OpenSoamViewer
            | OnKeyAction::OpenRomHeaderViewer
            | OnKeyAction::ChangeDebugPalette
            | OnKeyAction::Speedup => KeybindCategory::Debug,
            OnKeyAction::LoadRom
            | OnKeyAction::Quit
            | OnKeyAction::LoadSavestate
            | OnKeyAction::CreateSavestate
            | OnKeyAction::BrowseSavestates
            | OnKeyAction::OpenOptionsMenu
            | OnKeyAction::OpenKeybindingsMenu => KeybindCategory::Ui,
            OnKeyAction::PowerToggle | OnKeyAction::Reset | OnKeyAction::PowerCycle => {
                KeybindCategory::Console
            }
        }
    }

    /// Returns `true` when this action should pass through and co-trigger with
    /// other active bindings.
    ///
    /// When enabled, overlapping bindings can activate simultaneously instead
    /// of being filtered by specificity (for example, both `Shift` and
    /// `Shift+Enter` can trigger together).
    pub fn allows_multi_trigger(&self) -> bool {
        self.get_category() == KeybindCategory::Controller
    }

    /// Returns whether modifier matching for this action should permit extra
    /// held modifiers beyond those explicitly required by the binding.
    ///
    /// For example, a binding that requires `Shift` will still match while
    /// `Shift+Ctrl` is held when this returns `true`.
    pub fn allows_extra_modifiers(&self) -> bool {
        self.get_category() == KeybindCategory::Controller
    }

    pub fn get_associated_message(&self) -> AsyncFrontendMessage {
        match self {
            OnKeyAction::ControllerUp => AsyncFrontendMessage::ControllerInput(ControllerEvent::Up),
            OnKeyAction::ControllerDown => {
                AsyncFrontendMessage::ControllerInput(ControllerEvent::Down)
            }
            OnKeyAction::ControllerLeft => {
                AsyncFrontendMessage::ControllerInput(ControllerEvent::Left)
            }
            OnKeyAction::ControllerRight => {
                AsyncFrontendMessage::ControllerInput(ControllerEvent::Right)
            }
            OnKeyAction::ControllerAButton => {
                AsyncFrontendMessage::ControllerInput(ControllerEvent::A)
            }
            OnKeyAction::ControllerBButton => {
                AsyncFrontendMessage::ControllerInput(ControllerEvent::B)
            }
            OnKeyAction::ControllerStartButton => {
                AsyncFrontendMessage::ControllerInput(ControllerEvent::Start)
            }
            OnKeyAction::ControllerSelectButton => {
                AsyncFrontendMessage::ControllerInput(ControllerEvent::Select)
            }
            OnKeyAction::PauseEmulator => AsyncFrontendMessage::PauseEmulator,
            OnKeyAction::StepFrame => AsyncFrontendMessage::StepFrame,
            OnKeyAction::StepScanline => AsyncFrontendMessage::StepScanline,
            OnKeyAction::StepMasterCycle => AsyncFrontendMessage::StepMasterCycle,
            OnKeyAction::StepPpuCycle => AsyncFrontendMessage::StepPpuCycle,
            OnKeyAction::StepCpuCycle => AsyncFrontendMessage::StepCpuCycle,
            OnKeyAction::Reset => AsyncFrontendMessage::Reset,
            OnKeyAction::Quicksave => AsyncFrontendMessage::Quicksave,
            OnKeyAction::Quickload => AsyncFrontendMessage::Quickload,
            OnKeyAction::ChangeDebugPalette => AsyncFrontendMessage::ChangeDebugPalette,
            OnKeyAction::LoadRom => AsyncFrontendMessage::StartLoadRom,
            OnKeyAction::Quit => AsyncFrontendMessage::Quit,
            OnKeyAction::LoadSavestate => AsyncFrontendMessage::StartLoadSavestate,
            OnKeyAction::CreateSavestate => AsyncFrontendMessage::CreateSavestate,
            OnKeyAction::BrowseSavestates => AsyncFrontendMessage::OpenSaveBrowser,
            OnKeyAction::PowerCycle => AsyncFrontendMessage::PowerCycle,
            OnKeyAction::PowerToggle => AsyncFrontendMessage::PowerToggle,
            OnKeyAction::OpenOptionsMenu => AsyncFrontendMessage::OpenOptionsMenu,
            OnKeyAction::OpenKeybindingsMenu => AsyncFrontendMessage::OpenKeybindsMenu,
            OnKeyAction::OpenPaletteViewer => AsyncFrontendMessage::OpenPaletteViewer,
            OnKeyAction::OpenPatternTableViewer => AsyncFrontendMessage::OpenPatternTableViewer,
            OnKeyAction::OpenNametableViewer => AsyncFrontendMessage::OpenNametableViewer,
            OnKeyAction::OpenSpriteViewer => AsyncFrontendMessage::OpenSpriteViewer,
            OnKeyAction::OpenSoamViewer => AsyncFrontendMessage::OpenSoamViewer,
            OnKeyAction::OpenRomHeaderViewer => AsyncFrontendMessage::OpenRomHeaderViewer,
            OnKeyAction::Speedup => AsyncFrontendMessage::Speedup,
        }
    }

    pub fn get_callback_function(&self) -> Box<HotKeyCallback> {
        let message = self.get_associated_message();

        Box::new(move |sender| {
            let _ = sender.send(message.clone());
        })
    }
}

/// Binding to a variant that also stores the [`Modifiers`].
#[derive(Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
pub struct Binding {
    pub variant: BindVariant,
    pub modifiers: Modifiers,
    pub action: OnKeyAction,
}

impl Debug for Binding {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Binding")
            .field("variant", &self.variant)
            .field("modifiers", &self.modifiers)
            .finish()
    }
}

impl Binding {
    /// Create a new binding with just a key.
    pub const fn key(key: Key, action: OnKeyAction) -> Self {
        Self {
            variant: BindVariant::Keyboard(key),
            modifiers: Modifiers::NONE,
            action,
        }
    }

    /// Create a new binding with a key and modifiers.
    pub const fn with_modifiers(key: Key, modifiers: Modifiers, action: OnKeyAction) -> Self {
        Self {
            variant: BindVariant::Keyboard(key),
            modifiers,
            action,
        }
    }

    /// Format as a short string for display.
    pub fn get_string_rep(&self) -> String {
        let mut items = Vec::new();
        if self.modifiers.ctrl {
            items.push("Ctrl");
        }
        if self.modifiers.alt {
            items.push("Alt");
        }
        if self.modifiers.shift {
            items.push("Shift");
        }

        let merged = items.join("+");
        if merged.is_empty() {
            self.variant.to_string()
        } else {
            format!("{merged}+{}", self.variant)
        }
    }

    /// Returns true if the variant was pressed and input modifiers are
    /// matching.
    ///
    /// Note: always returns `false` for [`ModifierKey`] bindings — see
    /// [`BindVariant::pressed`] for details.
    pub fn pressed(&self, input_state: &InputState) -> bool {
        self.pressed_with_modifier_matching(input_state, false)
    }

    fn pressed_with_modifier_matching(
        &self,
        input_state: &InputState,
        allow_extra_modifiers: bool,
    ) -> bool {
        match &self.variant {
            // Modifier-only bindings cannot detect single-frame presses
            // because egui does not emit key events for modifiers.
            BindVariant::ModifierKey(_) => false,
            _ => {
                modifiers_match(input_state.modifiers, self.modifiers, allow_extra_modifiers)
                    && self.variant.pressed(input_state)
            }
        }
    }

    /// Returns true if the variant is down and input modifiers are matching.
    pub fn down(&self, input_state: &InputState) -> bool {
        self.down_with_modifier_matching(input_state, false)
    }

    fn down_with_modifier_matching(
        &self,
        input_state: &InputState,
        allow_extra_modifiers: bool,
    ) -> bool {
        match &self.variant {
            // Modifier-only bindings check the modifier flag directly, but
            // we also ensure that *no other* modifiers are active.
            //
            // We intentionally do not use `matches_logically` here, because
            // it would treat the active modifier itself (e.g. `ctrl` for a
            // Ctrl-only binding) as an "extra" modifier when
            // `self.modifiers == Modifiers::NONE`.
            BindVariant::ModifierKey(mk) => {
                debug_assert!(
                    self.modifiers == Modifiers::NONE,
                    "ModifierKey bindings are expected to use Modifiers::NONE; got {:?}",
                    self.modifiers,
                );

                let mods = input_state.modifiers;

                if allow_extra_modifiers {
                    mk.is_down(input_state)
                } else {
                    match mk {
                        ModifierKey::Shift => {
                            mods.shift && !mods.ctrl && !mods.alt && !mods.command && !mods.mac_cmd
                        }
                        ModifierKey::Ctrl => {
                            mods.ctrl && !mods.shift && !mods.alt && !mods.command && !mods.mac_cmd
                        }
                        ModifierKey::Alt => {
                            mods.alt && !mods.shift && !mods.ctrl && !mods.command && !mods.mac_cmd
                        }
                        ModifierKey::Command => {
                            mods.command && !mods.shift && !mods.ctrl && !mods.alt && !mods.mac_cmd
                        }
                        ModifierKey::MacCmd => {
                            mods.mac_cmd && !mods.shift && !mods.ctrl && !mods.alt && !mods.command
                        }
                    }
                }
            }
            BindVariant::Unbound => false,
            _ => {
                modifiers_match(input_state.modifiers, self.modifiers, allow_extra_modifiers)
                    && self.variant.down(input_state)
            }
        }
    }
}

/// Match current modifiers against required ones.
///
/// - Strict mode (`allow_extra_modifiers = false`) delegates to egui's
///   `matches_logically`, preserving existing single-action semantics.
/// - Permissive mode requires all requested modifiers to be down, while
///   allowing additional modifiers to also be held.
///
/// Permissive mode is used for continuous/controller bindings so overlapping
/// held inputs (e.g. `Shift` and `Shift+Enter`) can co-trigger instead of
/// making one action block the other.
fn modifiers_match(current: Modifiers, required: Modifiers, allow_extra_modifiers: bool) -> bool {
    if !allow_extra_modifiers {
        return current.matches_logically(required);
    }

    (!required.alt || current.alt)
        && (!required.ctrl || current.ctrl)
        && (!required.shift || current.shift)
        && (!required.command || current.command)
        && (!required.mac_cmd || current.mac_cmd)
}

// ============================================================================
// HotkeyBinding trait (ported from egui_hotkey)
// ============================================================================

/// This Trait defines types that can be used as hotkey's target.
pub trait HotkeyBinding {
    const ACCEPT_KEYBOARD: bool;
    const ACCEPT_MOUSE: bool;

    fn new(variant: BindVariant, modifiers: Modifiers, action: OnKeyAction) -> Self;
    fn get(&self) -> Option<&Binding>;
    fn as_string(&self) -> String;
    fn set(&mut self, variant: BindVariant, modifiers: Modifiers, action: OnKeyAction);
    fn clear(&mut self);
    fn active(&self, input: &InputState) -> bool;
}

impl HotkeyBinding for Binding {
    const ACCEPT_KEYBOARD: bool = true;
    const ACCEPT_MOUSE: bool = true;

    fn new(variant: BindVariant, modifiers: Modifiers, action: OnKeyAction) -> Self {
        Binding {
            variant,
            modifiers,
            action,
        }
    }

    fn get(&self) -> Option<&Binding> { Some(self) }

    fn as_string(&self) -> String { self.get_string_rep() }

    fn set(&mut self, variant: BindVariant, modifiers: Modifiers, action: OnKeyAction) {
        self.variant = variant;
        self.modifiers = modifiers;
        self.action = action;
    }

    fn clear(&mut self) {
        self.variant = BindVariant::Unbound;
        self.modifiers = Modifiers::NONE;
    }

    fn active(&self, input: &InputState) -> bool {
        let allow_extra_modifiers = self.action.allows_extra_modifiers();
        match self.action.get_trigger_type() {
            TriggerType::Single => {
                self.pressed_with_modifier_matching(input, allow_extra_modifiers)
            }
            TriggerType::Continuous => {
                self.down_with_modifier_matching(input, allow_extra_modifiers)
            }
        }
    }
}

impl<B> HotkeyBinding for Option<B>
where
    B: HotkeyBinding,
{
    const ACCEPT_KEYBOARD: bool = B::ACCEPT_KEYBOARD;
    const ACCEPT_MOUSE: bool = B::ACCEPT_MOUSE;

    fn new(variant: BindVariant, modifiers: Modifiers, action: OnKeyAction) -> Self {
        Some(B::new(variant, modifiers, action))
    }

    fn get(&self) -> Option<&Binding> { self.as_ref()?.get() }

    fn as_string(&self) -> String {
        match self {
            None => "Not Bound".to_string(),
            Some(b) => b.as_string(),
        }
    }

    fn set(&mut self, variant: BindVariant, modifiers: Modifiers, action: OnKeyAction) {
        if let Some(this) = self {
            this.set(variant, modifiers, action);
        } else {
            *self = Self::new(variant, modifiers, action);
        }
    }

    fn clear(&mut self) { *self = None; }

    fn active(&self, input: &InputState) -> bool {
        match self {
            Some(b) => b.active(input),
            None => false,
        }
    }
}

// ============================================================================
// Hotkey widget (ported from egui_hotkey, updated for egui 0.33)
// ============================================================================

/// The hotkey widget.
pub struct Hotkey<'a, B>
where
    B: HotkeyBinding,
{
    binding: &'a mut B,
    id: Id,
    tooltip: Option<bool>,
    accept_modifier_keys: bool,
}

impl<'a, B> Hotkey<'a, B>
where
    B: HotkeyBinding,
{
    /// Changes the default id for this widget.
    pub fn with_id(binding: &'a mut B, id_source: impl Hash) -> Self {
        Self {
            binding,
            id: Id::new(id_source),
            tooltip: None,
            accept_modifier_keys: true,
        }
    }
}

impl<B> Widget for Hotkey<'_, B>
where
    B: HotkeyBinding,
{
    fn ui(self, ui: &mut Ui) -> Response {
        let size = ui.spacing().interact_size;
        let (rect, mut response) = ui.allocate_exact_size(size * vec2(2.0, 1.0), Sense::click());

        let was_expecting = get_expecting(ui, self.id);
        let mut expecting = was_expecting;

        if response.clicked() {
            expecting = !expecting;
        }

        // When we first enter expecting mode, seed prev_mods with the current
        // modifier state so that modifiers already held at click time are not
        // immediately captured.
        if expecting && !was_expecting {
            let mods = ui.input(|i| i.modifiers);
            set_prev_modifiers(ui, self.id, mods);
        }

        if expecting {
            if response.clicked_elsewhere() {
                expecting = false;
            } else {
                let escape_pressed = ui.input(|i| i.key_pressed(Key::Escape));
                if escape_pressed {
                    self.binding.clear();
                    hotkey_set_suppressed_binding(ui.ctx(), None);
                    ui.ctx().memory_mut(|memory| {
                        memory
                            .data
                            .insert_temp(hotkey_just_set_this_frame_id(), true);
                    });
                    response.mark_changed();
                    expecting = false;
                    set_pending_modifier(ui, self.id, None);
                } else {
                    let keyboard = ui.input(|i| {
                        i.events.iter().find_map(|e| match e {
                            Event::Key {
                                key,
                                pressed: true,
                                modifiers,
                                ..
                            } => Some((BindVariant::Keyboard(*key), Some(*modifiers))),
                            _ => None,
                        })
                    });
                    let keyboard = if B::ACCEPT_KEYBOARD { keyboard } else { None };

                    let mouse = ui.input(|i| {
                        i.events.iter().find_map(|e| match e {
                            Event::PointerButton {
                                button,
                                pressed: true,
                                modifiers,
                                ..
                            } if *button != PointerButton::Primary
                                && *button != PointerButton::Secondary =>
                            {
                                Some((BindVariant::Mouse(*button), Some(*modifiers)))
                            }
                            _ => None,
                        })
                    });
                    let mouse = if B::ACCEPT_MOUSE { mouse } else { None };

                    let action = self
                        .binding
                        .get()
                        .expect("Binding does not have associated Action")
                        .action;

                    if let Some((key, mods)) = keyboard.or(mouse) {
                        let (key, mods) =
                            normalize_captured_binding(key, mods.unwrap_or(Modifiers::NONE));
                        self.binding.set(key, mods, action);
                        hotkey_set_suppressed_binding(
                            ui.ctx(),
                            Some(SuppressedBinding {
                                action,
                                binding: Binding {
                                    variant: key,
                                    modifiers: mods,
                                    action,
                                },
                            }),
                        );
                        ui.ctx().memory_mut(|memory| {
                            memory
                                .data
                                .insert_temp(hotkey_just_set_this_frame_id(), true);
                        });
                        response.mark_changed();
                        expecting = false;
                        set_pending_modifier(ui, self.id, None);
                    } else if B::ACCEPT_KEYBOARD && self.accept_modifier_keys {
                        // No regular key or mouse event. For modifier-only
                        // bindings we defer commit until modifier release, so
                        // modifier-first combos (e.g. Ctrl+K) can be captured.
                        let prev_mods = get_prev_modifiers(ui, self.id);
                        let current_mods = ui.input(|i| i.modifiers);
                        let pending_modifier = get_pending_modifier(ui, self.id);
                        // Always update prev_mods for the next frame so we
                        // track releases and detect the next down edge.
                        set_prev_modifiers(ui, self.id, current_mods);

                        if let Some(mk) = pending_modifier {
                            if !modifier_is_down(mk, current_mods) {
                                self.binding.set(
                                    BindVariant::ModifierKey(mk),
                                    Modifiers::NONE,
                                    action,
                                );
                                hotkey_set_suppressed_binding(
                                    ui.ctx(),
                                    Some(SuppressedBinding {
                                        action,
                                        binding: Binding {
                                            variant: BindVariant::ModifierKey(mk),
                                            modifiers: Modifiers::NONE,
                                            action,
                                        },
                                    }),
                                );
                                ui.ctx().memory_mut(|memory| {
                                    memory
                                        .data
                                        .insert_temp(hotkey_just_set_this_frame_id(), true);
                                });
                                response.mark_changed();
                                expecting = false;
                                set_pending_modifier(ui, self.id, None);
                            }
                        } else if let Some(mk) = newly_pressed_modifier(prev_mods, current_mods) {
                            set_pending_modifier(ui, self.id, Some(mk));
                        }
                    }
                }
            }
        } else if let Some(bind) = self.binding.get()
            && self.tooltip.unwrap_or(true)
        {
            response = response.on_hover_text(bind.get_string_rep());
        }

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact_selectable(&response, expecting);
            ui.painter()
                .rect_filled(rect, visuals.corner_radius, visuals.bg_fill);

            let text = self
                .binding
                .get()
                .map(|hk| hk.get_string_rep())
                .unwrap_or_else(|| "None".into());

            ui.painter().text(
                rect.center() + vec2(0.0, 1.0),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::default(),
                visuals.text_color(),
            );
        }

        set_expecting(ui, self.id, expecting);
        response
    }
}

fn get_expecting(ui: &Ui, id: Id) -> bool {
    ui.ctx()
        .memory_mut(|memory| memory.data.get_temp::<Id>(hotkey_active_id()) == Some(id))
}

fn set_expecting(ui: &Ui, id: Id, new: bool) {
    ui.ctx().memory_mut(|memory| {
        if new {
            memory.data.insert_temp(hotkey_active_id(), id);
        } else if memory.data.get_temp::<Id>(hotkey_active_id()) == Some(id) {
            memory.data.remove::<Id>(hotkey_active_id());
        }
    });

    if !new {
        set_pending_modifier(ui, id, None);
    }
}

/// Store the modifier state from the previous frame while the Hotkey widget
/// is in expecting mode.  Comparing the current modifiers against these lets
/// us detect up→down edge transitions: a modifier that was held before
/// clicking can be released and re-pressed and will still be captured.
fn set_prev_modifiers(ui: &Ui, id: Id, mods: Modifiers) {
    let storage_id = Id::new("hotkey_prev_mods").with(ui.make_persistent_id(id));
    ui.ctx()
        .memory_mut(|memory| memory.data.insert_temp(storage_id, mods));
}

fn get_prev_modifiers(ui: &Ui, id: Id) -> Modifiers {
    let storage_id = Id::new("hotkey_prev_mods").with(ui.make_persistent_id(id));
    ui.ctx()
        .memory_mut(|memory| memory.data.get_temp(storage_id).unwrap_or(Modifiers::NONE))
}

fn set_pending_modifier(ui: &Ui, id: Id, modifier: Option<ModifierKey>) {
    let storage_id = Id::new("hotkey_pending_modifier").with(ui.make_persistent_id(id));
    ui.ctx().memory_mut(|memory| {
        if let Some(modifier) = modifier {
            memory.data.insert_temp(storage_id, modifier);
        } else {
            memory.data.remove::<ModifierKey>(storage_id);
        }
    });
}

fn get_pending_modifier(ui: &Ui, id: Id) -> Option<ModifierKey> {
    let storage_id = Id::new("hotkey_pending_modifier").with(ui.make_persistent_id(id));
    ui.ctx()
        .memory_mut(|memory| memory.data.get_temp(storage_id))
}

/// Return the first modifier key that is active in `current` but was *not*
/// active in `initial`.  Returns `None` if no new modifier was pressed.
fn newly_pressed_modifier(initial: Modifiers, current: Modifiers) -> Option<ModifierKey> {
    if current.shift && !initial.shift {
        Some(ModifierKey::Shift)
    } else if current.ctrl && !initial.ctrl {
        Some(ModifierKey::Ctrl)
    } else if current.alt && !initial.alt {
        Some(ModifierKey::Alt)
    } else if current.command && !initial.command {
        Some(ModifierKey::Command)
    } else if current.mac_cmd && !initial.mac_cmd {
        Some(ModifierKey::MacCmd)
    } else {
        None
    }
}

fn modifier_is_down(modifier: ModifierKey, modifiers: Modifiers) -> bool {
    match modifier {
        ModifierKey::Shift => modifiers.shift,
        ModifierKey::Ctrl => modifiers.ctrl,
        ModifierKey::Alt => modifiers.alt,
        ModifierKey::Command => modifiers.command,
        ModifierKey::MacCmd => modifiers.mac_cmd,
    }
}

/// Normalize a just-captured binding for conventional keybind display.
///
/// On some platforms/layouts, pressing `Shift + <base-key>` yields a shifted
/// symbol key variant (e.g. `Questionmark`) instead of the physical base key
/// (`Slash`) with `shift = true`. We normalize such captures to base-key plus
/// modifier so bindings are stored and shown in the common `Shift+/` style.
fn normalize_captured_binding(
    variant: BindVariant,
    modifiers: Modifiers,
) -> (BindVariant, Modifiers) {
    match variant {
        BindVariant::Keyboard(key) => {
            let normalized_key = normalize_shifted_key(key, modifiers.shift);
            (BindVariant::Keyboard(normalized_key), modifiers)
        }
        _ => (variant, modifiers),
    }
}

fn normalize_shifted_key(key: Key, shift_held: bool) -> Key {
    if !shift_held {
        return key;
    }

    // Normalize only keys with a clear unshifted counterpart in egui's Key
    // enum. Layout-dependent symbols that do not map to a distinct base key
    // variant are intentionally left unchanged.
    match key {
        Key::Questionmark => Key::Slash,
        Key::Plus => Key::Equals,
        Key::Colon => Key::Semicolon,
        Key::OpenCurlyBracket => Key::OpenBracket,
        Key::CloseCurlyBracket => Key::CloseBracket,
        Key::Pipe => Key::Backslash,
        Key::Exclamationmark => Key::Num1,
        _ => key,
    }
}
