use crossbeam_channel::Sender;
use egui::Context;
use web_time::Instant;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::keybindings::Binding;
use crate::frontend::messages::AsyncFrontendMessage;
use crate::messages::ControllerEvent;

/// Check if a keybind is pressed in the given input state.
fn is_binding_pressed(input: &egui::InputState, binding: &Option<Binding>) -> bool {
    match binding {
        Some(b) => b.pressed(input),
        None => false,
    }
}

/// Check if a keybind is currently held down.
///
/// Unlike [`is_binding_pressed`], this returns true every frame the key is held,
/// supports multiple simultaneous keys, and has no OS text-input repeat delay.
/// This is appropriate for controller inputs where immediate, continuous response
/// is needed.
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
    last_frame_request: &mut Instant,
) {
    ctx.input(|i| {
        // Debug controls
        if is_binding_pressed(i, &config.keybindings.debug.cycle_palette) {
            config.view_config.debug_active_palette += 1;
            config.view_config.debug_active_palette &= 7;
        }

        // Emulation controls
        if is_binding_pressed(i, &config.keybindings.emulation.pause) {
            config.speed_config.is_paused = !config.speed_config.is_paused;
            *last_frame_request = Instant::now();
        }

        if is_binding_pressed(i, &config.keybindings.emulation.step_frame) {
            let _ = async_sender.send(AsyncFrontendMessage::StepFrame);
        }

        if is_binding_pressed(i, &config.keybindings.emulation.step_scanline) {
            let _ = async_sender.send(AsyncFrontendMessage::StepScanline);
        }

        if is_binding_pressed(i, &config.keybindings.emulation.step_master_cycle) {
            let _ = async_sender.send(AsyncFrontendMessage::StepMasterCycle);
        }

        if is_binding_pressed(i, &config.keybindings.emulation.step_cpu_cycle) {
            let _ = async_sender.send(AsyncFrontendMessage::StepCpuCycle);
        }

        if is_binding_pressed(i, &config.keybindings.emulation.step_ppu_cycle) {
            let _ = async_sender.send(AsyncFrontendMessage::StepPpuCycle);
        }

        if is_binding_pressed(i, &config.keybindings.emulation.reset) {
            let _ = async_sender.send(AsyncFrontendMessage::Reset);
        }

        if is_binding_pressed(i, &config.keybindings.emulation.quicksave) {
            let _ = async_sender.send(AsyncFrontendMessage::Quicksave);
        }

        if is_binding_pressed(i, &config.keybindings.emulation.quickload) {
            let _ = async_sender.send(AsyncFrontendMessage::Quickload);
        }

        // NES controller input
        handle_controller_input(i, async_sender, config);
    });
}

/// Handle NES controller input mapping from keyboard
fn handle_controller_input(
    input: &egui::InputState,
    async_sender: &Sender<AsyncFrontendMessage>,
    config: &AppConfig,
) {
    // D-pad — use is_binding_down for immediate, continuous input without
    // OS text-input repeat delay, and to allow multiple simultaneous keys.
    if is_binding_down(input, &config.keybindings.controller.left) {
        let _ = async_sender.send(AsyncFrontendMessage::ControllerInput(ControllerEvent::Left));
    }
    if is_binding_down(input, &config.keybindings.controller.right) {
        let _ = async_sender.send(AsyncFrontendMessage::ControllerInput(
            ControllerEvent::Right,
        ));
    }
    if is_binding_down(input, &config.keybindings.controller.up) {
        let _ = async_sender.send(AsyncFrontendMessage::ControllerInput(ControllerEvent::Up));
    }
    if is_binding_down(input, &config.keybindings.controller.down) {
        let _ = async_sender.send(AsyncFrontendMessage::ControllerInput(ControllerEvent::Down));
    }

    // Buttons
    if is_binding_down(input, &config.keybindings.controller.start) {
        let _ = async_sender.send(AsyncFrontendMessage::ControllerInput(
            ControllerEvent::Start,
        ));
    }
    if is_binding_down(input, &config.keybindings.controller.select) {
        let _ = async_sender.send(AsyncFrontendMessage::ControllerInput(
            ControllerEvent::Select,
        ));
    }
    if is_binding_down(input, &config.keybindings.controller.a) {
        let _ = async_sender.send(AsyncFrontendMessage::ControllerInput(ControllerEvent::A));
    }
    if is_binding_down(input, &config.keybindings.controller.b) {
        let _ = async_sender.send(AsyncFrontendMessage::ControllerInput(ControllerEvent::B));
    }
}
