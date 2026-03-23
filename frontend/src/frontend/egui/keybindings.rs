//! Keybindings configuration for the emulator.
//!
//! This module provides configurable keybindings for:
//! - NES controller inputs (Up, Down, Left, Right, A, B, Start, Select)
//! - Emulation controls (Pause, Reset, Quicksave, Quickload)
//! - Debug controls (Cycle palette)
//!
//! This module includes a port of the egui_hotkey crate's functionality
//! updated to work with egui 0.33.

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::ops::Deref;

use egui::{
    Event, Id, InputState, Key, Modifiers, PointerButton, Response, Sense, Ui, Widget, vec2,
};
use serde::{Deserialize, Serialize};

/// Well-known egui data ID used to signal that a [`Hotkey`] widget is
/// currently waiting for the user to press a key.  The input handler
/// checks this flag so it can skip consuming key events while the
/// keybinding UI is active.
pub(crate) fn hotkey_expecting_id() -> Id { Id::new("hotkey_expecting_input") }

// ============================================================================
// Binding types (ported from egui_hotkey)
// ============================================================================

/// A standalone modifier key (Shift, Ctrl, or Alt) used as a binding target.
///
/// Unlike regular [`Key`] variants, modifier keys are not part of egui's `Key`
/// enum and are only tracked via [`Modifiers`].  This variant allows users to
/// bind a bare modifier key to a controller button.
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
    Mouse(PointerButton),
    Keyboard(Key),
    /// A bare modifier key (e.g., Shift / Ctrl / Alt / Command / MacCmd) used on its own.
    ModifierKey(ModifierKey),
}

impl BindVariant {
    /// Returns true if the variant was pressed.
    ///
    /// Note: [`ModifierKey`] bindings always return `false` here because egui
    /// does not emit single-frame press events for modifier keys.  Use
    /// [`down`](Self::down) for continuous (held) checks instead, which is
    /// what controller inputs use.
    pub fn pressed(&self, input_state: impl Deref<Target = InputState>) -> bool {
        match self {
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
    pub fn down(&self, input_state: impl Deref<Target = InputState>) -> bool {
        match self {
            BindVariant::Mouse(mb) => input_state.pointer.button_down(*mb),
            BindVariant::Keyboard(kb) => input_state.key_down(*kb),
            BindVariant::ModifierKey(mk) => mk.is_down(&input_state),
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

/// Binding to a variant that also stores the [`Modifiers`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub variant: BindVariant,
    pub modifiers: Modifiers,
}

impl Default for Binding {
    fn default() -> Self {
        Self {
            variant: BindVariant::Keyboard(Key::Space),
            modifiers: Modifiers::NONE,
        }
    }
}

impl Binding {
    /// Create a new binding with just a key.
    pub const fn key(key: Key) -> Self {
        Self {
            variant: BindVariant::Keyboard(key),
            modifiers: Modifiers::NONE,
        }
    }

    /// Create a new binding with a key and modifiers.
    pub const fn with_modifiers(key: Key, modifiers: Modifiers) -> Self {
        Self {
            variant: BindVariant::Keyboard(key),
            modifiers,
        }
    }

    /// Format as a short string for display.
    pub fn into_string(self) -> String {
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

    /// Returns true if the variant was pressed and input modifiers are matching.
    ///
    /// Note: always returns `false` for [`ModifierKey`] bindings — see
    /// [`BindVariant::pressed`] for details.
    pub fn pressed(&self, input_state: impl Deref<Target = InputState>) -> bool {
        match &self.variant {
            // Modifier-only bindings cannot detect single-frame presses
            // because egui does not emit key events for modifiers.
            BindVariant::ModifierKey(_) => false,
            _ => {
                input_state.modifiers.matches_logically(self.modifiers)
                    && self.variant.pressed(input_state)
            }
        }
    }

    /// Returns true if the variant is down and input modifiers are matching.
    pub fn down(&self, input_state: impl Deref<Target = InputState>) -> bool {
        match &self.variant {
            // Modifier-only bindings check the modifier flag directly.
            // We bypass `matches_logically` because it would reject
            // e.g. a Ctrl-only binding (Ctrl sets `modifiers.ctrl`, and
            // `matches_logically(NONE)` considers extra Ctrl an error).
            BindVariant::ModifierKey(mk) => mk.is_down(&input_state),
            _ => {
                input_state.modifiers.matches_logically(self.modifiers)
                    && self.variant.down(input_state)
            }
        }
    }
}

// ============================================================================
// HotkeyBinding trait (ported from egui_hotkey)
// ============================================================================

/// This Trait defines types that can be used as hotkey's target.
pub trait HotkeyBinding {
    const ACCEPT_MOUSE: bool;
    const ACCEPT_KEYBOARD: bool;

    fn new(variant: BindVariant, modifiers: Modifiers) -> Self;
    fn get(&self) -> Option<Binding>;
    fn set(&mut self, variant: BindVariant, modifiers: Modifiers);
    fn clear(&mut self);
}

impl HotkeyBinding for Binding {
    const ACCEPT_KEYBOARD: bool = true;
    const ACCEPT_MOUSE: bool = true;

    fn new(variant: BindVariant, modifiers: Modifiers) -> Self {
        Binding {
            variant,
            modifiers,
        }
    }

    fn get(&self) -> Option<Binding> { Some(*self) }

    fn set(&mut self, variant: BindVariant, modifiers: Modifiers) {
        self.variant = variant;
        self.modifiers = modifiers;
    }

    fn clear(&mut self) {
        panic!(
            "Binding cannot be cleared directly. Use Option<Binding> wrapper type if clearing is needed, as Binding always requires a value."
        );
    }
}

impl<B> HotkeyBinding for Option<B>
where
    B: HotkeyBinding,
{
    const ACCEPT_KEYBOARD: bool = B::ACCEPT_KEYBOARD;
    const ACCEPT_MOUSE: bool = B::ACCEPT_MOUSE;

    fn new(variant: BindVariant, modifiers: Modifiers) -> Self { Some(B::new(variant, modifiers)) }

    fn get(&self) -> Option<Binding> { self.as_ref()?.get() }

    fn set(&mut self, variant: BindVariant, modifiers: Modifiers) {
        if let Some(this) = self {
            this.set(variant, modifiers);
        } else {
            *self = Self::new(variant, modifiers);
        }
    }

    fn clear(&mut self) { *self = None; }
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
        }
    }
}

impl<B> Widget for Hotkey<'_, B>
where
    B: HotkeyBinding,
{
    fn ui(self, ui: &mut Ui) -> Response {
        let size = ui.spacing().interact_size;
        let (rect, mut response) = ui.allocate_exact_size(size * vec2(1.5, 1.), Sense::click());

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
                    expecting = false;
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

                    if let Some((key, mods)) = keyboard.or(mouse) {
                        self.binding.set(key, mods.unwrap_or(Modifiers::NONE));
                        response.mark_changed();
                        expecting = false;
                    } else if B::ACCEPT_KEYBOARD {
                        // No regular key or mouse event — check whether a
                        // modifier key was pressed on its own (Shift, Ctrl,
                        // or Alt).  Compare against the modifiers from the
                        // *previous frame* so that releasing and re-pressing
                        // a modifier that was held during the initial click
                        // is correctly detected as a new press (up→down edge).
                        let prev_mods = get_prev_modifiers(ui, self.id);
                        let current_mods = ui.input(|i| i.modifiers);
                        // Always update prev_mods for the next frame so we
                        // track releases and detect the next down edge.
                        set_prev_modifiers(ui, self.id, current_mods);
                        if let Some(mk) = newly_pressed_modifier(prev_mods, current_mods) {
                            self.binding
                                .set(BindVariant::ModifierKey(mk), Modifiers::NONE);
                            response.mark_changed();
                            expecting = false;
                        }
                    }
                }
            }
        } else if let Some(bind) = self.binding.get()
            && self.tooltip.unwrap_or(true)
        {
            response = response.on_hover_text(bind.into_string());
        }

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact_selectable(&response, expecting);
            ui.painter()
                .rect_filled(rect, visuals.corner_radius, visuals.bg_fill);

            let binding = self.binding.get();

            let text = binding
                .map(|hk| hk.into_string())
                .unwrap_or_else(|| "None".into());

            ui.painter().text(
                rect.center() + vec2(0., 1.),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::default(),
                visuals.text_color(),
            );
        }

        // Signal the input handler that a hotkey widget is waiting for
        // a key press so it should not consume key events this frame.
        if expecting {
            ui.ctx()
                .data_mut(|d| d.insert_temp(hotkey_expecting_id(), true));
        }

        set_expecting(ui, self.id, expecting);
        response
    }
}

fn get_expecting(ui: &Ui, id: Id) -> bool {
    ui.ctx().memory_mut(|memory| {
        *memory
            .data
            .get_temp_mut_or_default::<bool>(ui.make_persistent_id(id))
    })
}

fn set_expecting(ui: &Ui, id: Id, new: bool) {
    ui.ctx().memory_mut(|memory| {
        *memory
            .data
            .get_temp_mut_or_default::<bool>(ui.make_persistent_id(id)) = new;
    });
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

// ============================================================================
// Keybindings configuration
// ============================================================================

/// Keybindings for NES controller (Player 1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControllerKeybindings {
    pub up: Option<Binding>,
    pub down: Option<Binding>,
    pub left: Option<Binding>,
    pub right: Option<Binding>,
    pub a: Option<Binding>,
    pub b: Option<Binding>,
    pub start: Option<Binding>,
    pub select: Option<Binding>,
}

impl Default for ControllerKeybindings {
    fn default() -> Self {
        Self {
            up: Some(Binding::key(Key::W)),
            down: Some(Binding::key(Key::S)),
            left: Some(Binding::key(Key::A)),
            right: Some(Binding::key(Key::D)),
            a: Some(Binding::key(Key::Space)),
            b: Some(Binding::new(
                BindVariant::ModifierKey(ModifierKey::Shift),
                Modifiers::NONE,
            )),
            start: Some(Binding::key(Key::Enter)),
            select: Some(Binding::key(Key::Tab)),
        }
    }
}

/// Keybindings for emulation controls
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmulationKeybindings {
    pub pause: Option<Binding>,
    pub step_frame: Option<Binding>,
    pub step_scanline: Option<Binding>,
    pub step_cpu_cycle: Option<Binding>,
    pub step_ppu_cycle: Option<Binding>,
    pub step_master_cycle: Option<Binding>,
    pub reset: Option<Binding>,
    pub quicksave: Option<Binding>,
    pub quickload: Option<Binding>,
}

impl Default for EmulationKeybindings {
    fn default() -> Self {
        Self {
            pause: Some(Binding::key(Key::Comma)),
            step_frame: Some(Binding::key(Key::Period)),
            step_scanline: Some(Binding::with_modifiers(Key::Period, Modifiers::CTRL)),
            step_master_cycle: Some(Binding::key(Key::Slash)),
            step_cpu_cycle: Some(Binding::with_modifiers(Key::Slash, Modifiers::ALT)),
            step_ppu_cycle: Some(Binding::with_modifiers(Key::Slash, Modifiers::SHIFT)),
            reset: Some(Binding::key(Key::R)),
            quicksave: Some(Binding::key(Key::F5)),
            quickload: Some(Binding::key(Key::F8)),
        }
    }
}

/// Keybindings for debug controls
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DebugKeybindings {
    pub cycle_palette: Option<Binding>,
}

impl Default for DebugKeybindings {
    fn default() -> Self {
        Self {
            cycle_palette: Some(Binding::key(Key::N)),
        }
    }
}

/// All keybindings for the emulator
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct KeybindingsConfig {
    pub controller: ControllerKeybindings,
    pub emulation: EmulationKeybindings,
    pub debug: DebugKeybindings,
}

impl KeybindingsConfig {
    /// Reset all keybindings to defaults.
    pub fn reset_to_defaults(&mut self) { *self = Self::default(); }
}
