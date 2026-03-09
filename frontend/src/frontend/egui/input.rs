use crossbeam_channel::Sender;
use egui::Context;
use web_time::Instant;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::keybindings::Binding;
use crate::frontend::messages::AsyncFrontendMessage;

/// Check if a keybind is pressed in the given input state.
fn is_binding_pressed(input: &egui::InputState, binding: &Option<Binding>) -> bool {
    match binding {
        Some(b) => b.pressed(input),
        None => false,
    }
}

/// Check if a keybind is currently held down.
fn is_binding_down(input: &egui::InputState, binding: &Option<Binding>) -> bool {
    match binding {
        Some(b) => b.down(input),
        None => false,
    }
}

/// Handle keyboard input from the user.
///
/// Returns the current NES controller state as an 8-bit value where each bit
/// represents a button: A(0), B(1), Select(2), Start(3), Up(4), Down(5),
/// Left(6), Right(7). The state is rebuilt each frame using key-down checks,
/// so buttons are active while held and released when the key is lifted.
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
) -> u8 {
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

        // NES controller input - compute full state from currently held keys
        compute_controller_state(i, config)
    })
}

/// Compute the NES controller state from currently held keys.
///
/// Uses `down()` (key-held) checks instead of `pressed()` (key-event) checks,
/// so the controller state accurately reflects which buttons are currently held.
/// The state is rebuilt from scratch each frame, ensuring buttons are released
/// when keys are lifted.
fn compute_controller_state(input: &egui::InputState, config: &AppConfig) -> u8 {
    let mut state = 0u8;

    if is_binding_down(input, &config.keybindings.controller.a) {
        state |= 1;
    }
    if is_binding_down(input, &config.keybindings.controller.b) {
        state |= 2;
    }
    if is_binding_down(input, &config.keybindings.controller.select) {
        state |= 4;
    }
    if is_binding_down(input, &config.keybindings.controller.start) {
        state |= 8;
    }
    if is_binding_down(input, &config.keybindings.controller.up) {
        state |= 16;
    }
    if is_binding_down(input, &config.keybindings.controller.down) {
        state |= 32;
    }
    if is_binding_down(input, &config.keybindings.controller.left) {
        state |= 64;
    }
    if is_binding_down(input, &config.keybindings.controller.right) {
        state |= 128;
    }

    state
}
