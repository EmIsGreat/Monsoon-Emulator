use crossbeam_channel::Sender;
use egui::{Context, FocusDirection};

use crate::frontend::egui::config::{AppConfig, KeybindingsConfig};
use crate::frontend::egui::keybindings::{
    BindVariant, Binding, HotkeyBinding, hotkey_get_suppressed_binding, hotkey_is_any_expecting,
    hotkey_set_suppressed_binding, hotkey_take_just_set_this_frame,
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
    let hotkey_just_set_this_frame = hotkey_take_just_set_this_frame(ctx);
    let mut suppressed_binding = hotkey_get_suppressed_binding(ctx);

    if let Some(suppressed) = suppressed_binding
        && !ctx.input(|i| suppressed.binding.down(i))
    {
        hotkey_set_suppressed_binding(ctx, None);
        suppressed_binding = None;
    }

    ctx.input_mut(|i| {
        if !hotkey_is_expecting && !hotkey_just_set_this_frame {
            let (actions, consumed_bindings) =
                resolve_active_key_actions(&config.keybindings, i, suppressed_binding);

            // Now no borrow of `config.keybindings` is active
            for action in actions {
                action.send(async_sender);
            }

            // Consume key events for non-controller active keybindings so that
            // egui widgets do not act on them (e.g. Space clicking a focused
            // button).  Skip this when the Hotkey rebinding widget is waiting
            // for a key press – it needs to see the raw events.
            consume_triggered_keys(i, &consumed_bindings);
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

/// Resolve active key actions for this frame.
///
/// Controller actions pass through and can co-trigger.
/// Non-controller actions are filtered to only those with the highest
/// specificity among currently active non-controller bindings.
fn resolve_active_key_actions(
    keybindings: &KeybindingsConfig,
    input: &egui::InputState,
    suppressed_binding: Option<crate::frontend::egui::keybindings::SuppressedBinding>,
) -> (
    Vec<crate::frontend::egui::keybindings::OnKeyAction>,
    Vec<Binding>,
) {
    let mut actions = Vec::new();
    let mut consumed_bindings = Vec::new();

    let mut highest_non_controller_specificity: Option<usize> = None;
    let mut pending_non_controller: Vec<(
        crate::frontend::egui::keybindings::OnKeyAction,
        Binding,
        usize,
    )> = Vec::new();

    for (action, binding) in &keybindings.keybindings {
        if let Some(suppressed) = suppressed_binding
            && *action == suppressed.action
            && *binding == suppressed.binding
        {
            continue;
        }

        if !binding.active(input) {
            continue;
        }

        if action.allows_multi_trigger() {
            actions.push(*action);
            continue;
        }

        let specificity = binding_specificity(binding);
        match highest_non_controller_specificity {
            None => {
                highest_non_controller_specificity = Some(specificity);
                pending_non_controller.push((*action, *binding, specificity));
            }
            Some(best) if specificity > best => {
                highest_non_controller_specificity = Some(specificity);
                pending_non_controller.clear();
                pending_non_controller.push((*action, *binding, specificity));
            }
            Some(best) if specificity == best => {
                pending_non_controller.push((*action, *binding, specificity));
            }
            Some(_) => {}
        }
    }

    for (action, binding, _) in pending_non_controller {
        actions.push(action);
        consumed_bindings.push(binding);
    }

    (actions, consumed_bindings)
}

/// Consume key-press events for the resolved triggered keybindings.
///
/// After the emulator's input handler has read the key state, we remove the
/// corresponding `Event::Key` entries from [`egui::InputState`] so that egui
/// widgets rendered later in the frame do not also react to them (e.g. Space
/// clicking a focused button, or Tab advancing widget focus).
fn consume_triggered_keys(input: &mut egui::InputState, consumed_bindings: &[Binding]) {
    for binding in consumed_bindings {
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

/// Returns a specificity score for tie-breaking overlapping active bindings.
///
/// Score = number of required modifier flags + 1 for non-modifier key/mouse
/// variants. Higher scores are considered more specific.
fn binding_specificity(binding: &Binding) -> usize {
    binding.modifiers.alt as usize
        + binding.modifiers.ctrl as usize
        + binding.modifiers.shift as usize
        + binding.modifiers.command as usize
        + binding.modifiers.mac_cmd as usize
        + match binding.variant {
            BindVariant::ModifierKey(_) => 0,
            BindVariant::Unbound => 0,
            _ => 1,
        }
}
