//! Keybindings pane rendering

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::keybindings::{Binding, Hotkey, KeybindCategory};

/// Render the keybindings panel
pub fn render_keybindings(ui: &mut egui::Ui, config: &mut AppConfig) -> bool {
    let mut changed = false;

    egui::ScrollArea::vertical().show(ui, |ui| {
        changed |= render_controller_keybindings(ui, &mut config.keybindings.keybindings);
        ui.separator();
        changed |= render_emulation_keybindings(ui, &mut config.keybindings.keybindings);
        ui.separator();
        changed |= render_reset_button(ui, config);
    });
    changed
}

/// Render NES controller keybindings section
fn render_controller_keybindings(ui: &mut egui::Ui, keybinds: &mut [Binding]) -> bool {
    let mut changed = false;
    ui.collapsing("Controller", |ui| {
        egui::Grid::new("controller_keybindings_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for keybinding in keybinds
                    .iter_mut()
                    .filter(|b| b.logical_bind.get_category() == KeybindCategory::Controller)
                {
                    ui.label(keybinding.logical_bind.get_display_name());
                    changed |= ui
                        .add(Hotkey::with_id(keybinding, keybinding.logical_bind))
                        .changed();
                    ui.end_row()
                }
            });
    });
    changed
}

/// Render emulation control keybindings section
fn render_emulation_keybindings(ui: &mut egui::Ui, keybinds: &mut [Binding]) -> bool {
    let mut changed = false;
    ui.collapsing("Emulation Controls", |ui| {
        egui::Grid::new("emulation_keybindings_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for keybinding in keybinds
                    .iter_mut()
                    .filter(|b| b.logical_bind.get_category() == KeybindCategory::Emulator)
                {
                    ui.label(keybinding.logical_bind.get_display_name());
                    changed |= ui
                        .add(
                            Hotkey::with_id(keybinding, keybinding.logical_bind)
                                .accept_modifier_keys(false),
                        )
                        .changed();
                    ui.end_row()
                }
            });
    });
    changed
}

/// Render reset to defaults button
fn render_reset_button(ui: &mut egui::Ui, config: &mut AppConfig) -> bool {
    if ui.button("Reset to Defaults").clicked() {
        config.keybindings.reset_to_defaults();
        return true;
    }
    false
}
