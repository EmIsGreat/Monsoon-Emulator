use crossbeam_channel::Sender;
use egui::{Context, FocusDirection};

use crate::frontend::egui::config::{AppConfig, KeybindingsConfig};
use crate::frontend::egui::keybindings::{
    BindVariant, Binding, HotkeyBinding, hotkey_is_any_expecting,
};
use crate::frontend::messages::AsyncFrontendMessage;

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
    let hotkey_is_expecting = hotkey_is_any_expecting(ctx);

    ctx.input_mut(|i| {
        for idx in 0..config.keybindings.keybindings.len() {
            let is_active = {
                let binding = &config.keybindings.keybindings[idx];
                binding.active(i)
            };

            if is_active {
                let action = config.keybindings.keybindings[idx].logical_bind;
                action.get_callback_function()(config, async_sender);
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
