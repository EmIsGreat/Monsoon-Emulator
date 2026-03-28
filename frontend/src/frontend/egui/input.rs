use crossbeam_channel::Sender;
use egui::{Context, FocusDirection};

use crate::frontend::egui::config::{AppConfig, KeybindingsConfig};
use crate::frontend::egui::keybindings::{
    BindVariant, Binding, HotkeyBinding, hotkey_expecting_id,
};
use crate::frontend::messages::AsyncFrontendMessage;

/// Check if a keybind is currently held down.
///
/// Unlike [`is_binding_pressed`], this returns true every frame the key is
/// held, supports multiple simultaneous keys, and has no OS text-input repeat
/// delay. This is appropriate for controller inputs where immediate, continuous
/// response is needed.
fn is_binding_down(input: &egui::InputState, binding: &Option<Binding>) -> bool {
    match binding {
        Some(b) => b.down(input),
        None => false,
    }
}

/// Handle keyboard input from the user.
///
/// # Arguments
/// * `ctx` - The egui context
/// * `async_sender` - Channel to send async messages
/// * `config` - Application configuration (modified for speed/view settings)
/// * `last_frame_request` - Last frame request time (reset when pausing)
pub fn handle_keyboard_input(
    ctx: &Context,
    async_sender: &Sender<AsyncFrontendMessage>,
    config: &mut AppConfig,
) {
    // Check whether a Hotkey widget is currently waiting for the user to
    // press a key (set during the *previous* frame's widget rendering).
    // When true we must let the raw key events through so the Hotkey
    // widget can capture them.
    let hotkey_is_expecting = ctx.data_mut(|d| {
        let val = d.get_temp::<bool>(hotkey_expecting_id()).unwrap_or(false);
        // Reset so the flag doesn't persist when no widget sets it.
        d.insert_temp(hotkey_expecting_id(), false);
        val
    });

    let bindings = config.keybindings.keybindings.clone();

    ctx.input_mut(|i| {
        for mut binding in bindings {
            if binding.active(&i) {
                binding.run_bound(config, async_sender)
            }
        }

        // Consume key events for all active keybindings so that egui
        // widgets do not act on them (e.g. Space clicking a focused
        // button).  Skip this when the Hotkey rebinding widget is
        // waiting for a key press – it needs to see the raw events.
        if !hotkey_is_expecting {
            consume_bound_keys(i, &config.keybindings);
        }
    });

    // Prevent egui's built-in focus-navigation from moving focus when
    // the user presses Tab or arrow keys that are bound to emulator
    // controls.  `Focus::begin_pass` has already set `focus_direction`
    // from those key events, so we reset it before any widgets run.
    if !hotkey_is_expecting {
        ctx.memory_mut(|m| m.move_focus(FocusDirection::None));
    }
}

/// Consume key-press events for every active keybinding.
///
/// After the emulator's input handler has read the key state, we remove the
/// corresponding `Event::Key` entries from [`egui::InputState`] so that egui
/// widgets rendered later in the frame do not also react to them (e.g. Space
/// clicking a focused button, or Tab advancing widget focus).
fn consume_bound_keys(input: &mut egui::InputState, keybindings: &KeybindingsConfig) {
    for binding in &keybindings.keybindings {
        consume_binding(input, &Some(*binding))
    }
}

/// Consume the key-press event for a single binding, if it is a keyboard
/// binding.  Mouse bindings are not consumed because they do not interfere
/// with egui's focus system.
fn consume_binding(input: &mut egui::InputState, binding: &Option<Binding>) {
    if let Some(b) = binding
        && let BindVariant::Keyboard(key) = b.variant
    {
        input.consume_key(b.modifiers, key);
    }
}
