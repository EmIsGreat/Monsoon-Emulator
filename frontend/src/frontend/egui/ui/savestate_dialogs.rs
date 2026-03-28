//! Dialogs for the savestate loading workflow.
//!
//! This module contains helper functions for rendering the various dialogs
//! that appear during the multistep savestate loading process.

use crossbeam_channel::Sender;

use crate::frontend::egui::config::{PendingDialogs, UserConfig};
use crate::frontend::messages::AsyncFrontendMessage;
use crate::frontend::savestates::RomSelectionDialogState;
use crate::frontend::util;

/// Helper to create a centered modal window
fn modal_window(title: &str) -> egui::Window<'_> {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
}

/// Extract ROM name from context or return "Unknown"
fn get_rom_name(state: &RomSelectionDialogState) -> String {
    state
        .context
        .savestate
        .rom_file
        .name
        .clone()
        .unwrap_or_else(|| "Unknown".to_string())
}

/// Render all savestate-related dialogs
pub fn render_dialogs(
    ctx: &egui::Context,
    dialogs: &mut PendingDialogs,
    user_config: &UserConfig,
    sender: &Sender<AsyncFrontendMessage>,
) {
    render_rom_selection_dialog(ctx, dialogs, user_config, sender);
    render_matching_rom_dialog(ctx, dialogs, sender);
    render_checksum_mismatch_dialog(ctx, dialogs, sender);
    render_error_dialog(ctx, dialogs);
}

/// ROM selection dialog - shown when no matching ROM was found
fn render_rom_selection_dialog(
    ctx: &egui::Context,
    dialogs: &mut PendingDialogs,
    user_config: &UserConfig,
    sender: &Sender<AsyncFrontendMessage>,
) {
    let Some(ref state) = dialogs.rom_selection_dialog else {
        return;
    };

    let rom_name = get_rom_name(state);
    let context = state.context.clone();
    let sender = sender.clone();

    let mut close = false;
    let mut spawn_picker = false;

    modal_window("Select ROM").show(ctx, |ui| {
        ui.label("Please select the ROM file for this savestate.");
        ui.add_space(8.0);
        ui.label(format!("Expected ROM: {}", rom_name));
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Select ROM...").clicked() {
                spawn_picker = true;
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
    });

    if close {
        if spawn_picker {
            util::spawn_rom_picker_for_savestate(
                &sender,
                context,
                user_config.previous_rom_load_dir.as_ref(),
            );
        }
        dialogs.rom_selection_dialog = None;
    }
}

/// Matching ROM dialog - shown when a matching ROM was found in the directory
fn render_matching_rom_dialog(
    ctx: &egui::Context,
    dialogs: &mut PendingDialogs,
    sender: &Sender<AsyncFrontendMessage>,
) {
    let Some(ref state) = dialogs.matching_rom_dialog else {
        return;
    };

    let rom_name = state.matching_rom.name.clone();
    let context = state.context.clone();
    let rom = state.matching_rom.clone();
    let sender = sender.clone();

    let mut close = false;
    let mut use_matching = false;
    let mut select_manual = false;

    modal_window("Matching ROM Found").show(ctx, |ui| {
        ui.label("A ROM file matching the savestate was found:");
        ui.add_space(8.0);
        ui.label(format!("ROM: {}", rom_name));
        ui.add_space(8.0);
        ui.label("Would you like to use this ROM or select one manually?");
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Use This ROM").clicked() {
                use_matching = true;
                close = true;
            }
            if ui.button("Select Manually...").clicked() {
                select_manual = true;
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
    });

    if close {
        if use_matching {
            let _ = sender.send(AsyncFrontendMessage::UseMatchingRom(context, rom));
        } else if select_manual {
            let _ = sender.send(AsyncFrontendMessage::ManuallySelectRom(context));
        }
        dialogs.matching_rom_dialog = None;
    }
}

/// Checksum mismatch dialog - shown when selected ROM doesn't match expected
/// checksum
fn render_checksum_mismatch_dialog(
    ctx: &egui::Context,
    dialogs: &mut PendingDialogs,
    sender: &Sender<AsyncFrontendMessage>,
) {
    let Some(ref state) = dialogs.checksum_mismatch_dialog else {
        return;
    };

    let expected_name = state
        .context
        .savestate
        .rom_file
        .name
        .clone()
        .unwrap_or_else(|| "Unknown".to_string());
    let selected_name = state.selected_rom.name.clone();
    let context = state.context.clone();
    let rom = state.selected_rom.clone();
    let sender = sender.clone();

    let mut close = false;
    let mut load_anyway = false;
    let mut select_another = false;

    modal_window("⚠ Checksum Mismatch").show(ctx, |ui| {
        ui.label("Warning: The selected ROM's checksum doesn't match!");
        ui.add_space(8.0);
        ui.label(format!("Expected ROM: {}", expected_name));
        ui.label(format!("Selected ROM: {}", selected_name));
        ui.add_space(8.0);
        ui.label("Loading a savestate with a different ROM may cause issues or crashes.");
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Load Anyway").clicked() {
                load_anyway = true;
                close = true;
            }
            if ui.button("Select Another ROM...").clicked() {
                select_another = true;
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
    });

    if close {
        if load_anyway {
            let _ = sender.send(AsyncFrontendMessage::LoadSavestateAnyway(context, rom));
        } else if select_another {
            let _ = sender.send(AsyncFrontendMessage::SelectAnotherRom(context));
        }
        dialogs.checksum_mismatch_dialog = None;
    }
}

/// Error dialog - shown when an error occurs during the loading process
fn render_error_dialog(ctx: &egui::Context, dialogs: &mut PendingDialogs) {
    let Some(ref state) = dialogs.error_dialog else {
        return;
    };

    let title = state.title.clone();
    let message = state.message.clone();

    let mut close = false;

    modal_window(&format!("❌ {}", title)).show(ctx, |ui| {
        ui.label(&message);
        ui.add_space(16.0);
        if ui.button("OK").clicked() {
            close = true;
        }
    });

    if close {
        dialogs.error_dialog = None;
    }
}
