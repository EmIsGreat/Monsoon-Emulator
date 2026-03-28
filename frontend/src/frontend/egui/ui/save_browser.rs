//! Save browser dialog for listing and loading internal saves.
//!
//! Shows quicksaves and autosaves stored in the application's storage
//! (filesystem on native, IndexedDB on WASM). Users can load saves directly
//! or export them to files on disk.

use crossbeam_channel::Sender;

use crate::frontend::egui::config::PendingDialogs;
use crate::frontend::messages::AsyncFrontendMessage;
use crate::frontend::savestates::SaveEntryType;

/// Render the save browser dialog if it's open
pub fn render_save_browser(
    ctx: &egui::Context,
    dialogs: &mut PendingDialogs,
    sender: &Sender<AsyncFrontendMessage>,
) {
    let Some(ref mut state) = dialogs.save_browser else {
        return;
    };

    let mut close = false;
    let mut load_key = None;
    let mut export_key = None;

    egui::Window::new("📂 Save Browser")
        .collapsible(false)
        .resizable(true)
        .default_width(450.0)
        .default_height(400.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("Saves for: {}", state.game_name));
            ui.add_space(4.0);

            // Filter checkboxes
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.show_quicksaves, "Quicksaves");
                ui.checkbox(&mut state.show_autosaves, "Autosaves");
            });
            ui.add_space(4.0);

            if state.loading {
                ui.spinner();
                ui.label("Loading saves...");
            } else {
                // Filter entries
                let filtered: Vec<_> = state
                    .entries
                    .iter()
                    .filter(|e| match e.save_type {
                        SaveEntryType::Quicksave => state.show_quicksaves,
                        SaveEntryType::Autosave => state.show_autosaves,
                    })
                    .collect();

                if filtered.is_empty() {
                    ui.label("No saves found.");
                } else {
                    ui.label(format!("{} save(s) found", filtered.len()));
                    ui.add_space(4.0);

                    // Scrollable save list
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for entry in &filtered {
                                ui.horizontal(|ui| {
                                    // Type badge
                                    let badge = match entry.save_type {
                                        SaveEntryType::Quicksave => "⚡",
                                        SaveEntryType::Autosave => "🔄",
                                    };

                                    ui.label(badge);

                                    // Name and timestamp
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&entry.display_name).strong());
                                        ui.label(
                                            egui::RichText::new(&entry.timestamp)
                                                .small()
                                                .color(ui.visuals().weak_text_color()),
                                        );
                                    });

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("Export").clicked() {
                                                export_key = Some(entry.key.clone());
                                            }
                                            if ui.button("Load").clicked() {
                                                load_key = Some(entry.key.clone());
                                            }
                                        },
                                    );
                                });
                                ui.separator();
                            }
                        });
                }
            }

            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                close = true;
            }
        });

    if let Some(key) = load_key {
        let _ = sender.send(AsyncFrontendMessage::LoadSaveFromBrowser(key));
        close = true;
    }

    if let Some(key) = export_key {
        let _ = sender.send(AsyncFrontendMessage::ExportSaveFromBrowser(key));
    }

    if close {
        dialogs.save_browser = None;
    }
}
