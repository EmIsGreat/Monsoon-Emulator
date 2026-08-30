//! Keybindings pane rendering

use std::collections::BTreeMap;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::keybindings::{BindVariant, Binding, Hotkey, OnKeyAction};
use crate::frontend::peripherals::StandardControllerBindings;

const STANDARD_CONTROLLER_ACTIONS: [OnKeyAction; 8] = [
    OnKeyAction::StdControllerUp,
    OnKeyAction::StdControllerDown,
    OnKeyAction::StdControllerLeft,
    OnKeyAction::StdControllerRight,
    OnKeyAction::StdControllerAButton,
    OnKeyAction::StdControllerBButton,
    OnKeyAction::StdControllerStartButton,
    OnKeyAction::StdControllerSelectButton,
];

const DEBUG_ACTIONS: [OnKeyAction; 18] = [
    OnKeyAction::PauseEmulator,
    OnKeyAction::StepFrame,
    OnKeyAction::StepScanline,
    OnKeyAction::StepMasterCycle,
    OnKeyAction::StepPpuCycle,
    OnKeyAction::StepCpuCycle,
    OnKeyAction::Quicksave,
    OnKeyAction::Quickload,
    OnKeyAction::ChangeDebugPalette,
    OnKeyAction::OpenPaletteViewer,
    OnKeyAction::OpenPatternTableViewer,
    OnKeyAction::OpenNametableViewer,
    OnKeyAction::OpenSpriteViewer,
    OnKeyAction::OpenSoamViewer,
    OnKeyAction::OpenRomHeaderViewer,
    OnKeyAction::OpenRegistersViewer,
    OnKeyAction::OpenTraceLogViewer,
    OnKeyAction::Speedup,
];

const UI_ACTIONS: [OnKeyAction; 7] = [
    OnKeyAction::LoadRom,
    OnKeyAction::Quit,
    OnKeyAction::LoadSavestate,
    OnKeyAction::CreateSavestate,
    OnKeyAction::BrowseSavestates,
    OnKeyAction::OpenOptionsMenu,
    OnKeyAction::OpenKeybindingsMenu,
];

const CONSOLE_ACTIONS: [OnKeyAction; 3] = [
    OnKeyAction::Reset,
    OnKeyAction::PowerCycle,
    OnKeyAction::PowerToggle,
];

#[derive(Clone, Copy)]
struct SharedLabelWidthCache {
    pixels_per_point: f32,
    key_count: usize,
    width: f32,
}

fn get_shared_label_column_width(ui: &mut egui::Ui) -> f32 {
    const PIXELS_PER_POINT_CACHE_TOLERANCE: f32 = 0.01;

    let cache_id = egui::Id::new("keybindings_shared_label_column_width");
    let pixels_per_point = ui.ctx().pixels_per_point();
    let key_count = STANDARD_CONTROLLER_ACTIONS.len()
        + DEBUG_ACTIONS.len()
        + UI_ACTIONS.len()
        + CONSOLE_ACTIONS.len();

    if let Some(cache) = ui
        .ctx()
        .memory_mut(|memory| memory.data.get_temp::<SharedLabelWidthCache>(cache_id))
        && (cache.pixels_per_point - pixels_per_point).abs() <= PIXELS_PER_POINT_CACHE_TOLERANCE
        && cache.key_count == key_count
    {
        return cache.width;
    }

    let label_font_id = egui::TextStyle::Body.resolve(ui.style());
    let label_color = ui.visuals().text_color();
    let width = STANDARD_CONTROLLER_ACTIONS
        .iter()
        .chain(DEBUG_ACTIONS.iter())
        .chain(UI_ACTIONS.iter())
        .chain(CONSOLE_ACTIONS.iter())
        .map(|action| {
            ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(
                        action.get_display_name().to_owned(),
                        label_font_id.clone(),
                        label_color,
                    )
                    .size()
                    .x
            })
        })
        .fold(0f32, f32::max);

    ui.ctx().memory_mut(|memory| {
        memory.data.insert_temp(
            cache_id,
            SharedLabelWidthCache {
                pixels_per_point,
                key_count,
                width,
            },
        );
    });

    width
}

/// Render the keybindings panel
pub fn render_keybindings(ui: &mut egui::Ui, config: &mut AppConfig) -> bool {
    let mut changed = false;
    let shared_label_column_width = get_shared_label_column_width(ui);

    egui::ScrollArea::vertical().show(ui, |ui| {
        changed |= render_standard_controller_bindings(
            ui,
            &mut config.keybindings.standard_controller,
            shared_label_column_width,
        );
        ui.separator();

        changed |= render_action_binding_group(
            ui,
            "Debug Keybinds",
            "debug_keybinds",
            &mut config.keybindings.debug,
            &DEBUG_ACTIONS,
            shared_label_column_width,
        );
        ui.separator();

        changed |= render_action_binding_group(
            ui,
            "Ui Shortcuts",
            "ui_keybinds",
            &mut config.keybindings.ui,
            &UI_ACTIONS,
            shared_label_column_width,
        );
        ui.separator();

        changed |= render_action_binding_group(
            ui,
            "Console Keybinds",
            "console_keybinds",
            &mut config.keybindings.console,
            &CONSOLE_ACTIONS,
            shared_label_column_width,
        );
        ui.separator();

        changed |= render_reset_button(ui, config);
    });
    changed
}

fn render_standard_controller_bindings(
    ui: &mut egui::Ui,
    bindings: &mut StandardControllerBindings,
    label_column_width: f32,
) -> bool {
    let mut changed = false;

    ui.collapsing("Standard Controller Bindings", |ui| {
        egui::Grid::new("standard_controller_keybinds")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for action in STANDARD_CONTROLLER_ACTIONS {
                    let binding = standard_controller_binding_mut(bindings, action);
                    ui.add_sized(
                        [label_column_width, ui.spacing().interact_size.y],
                        egui::Label::new(action.get_display_name()),
                    );
                    changed |= ui.add(Hotkey::with_id(binding, action)).changed();
                    ui.end_row();
                }
            });
    });

    changed
}

fn standard_controller_binding_mut(
    bindings: &mut StandardControllerBindings,
    action: OnKeyAction,
) -> &mut Binding {
    match action {
        OnKeyAction::StdControllerUp => &mut bindings.up,
        OnKeyAction::StdControllerDown => &mut bindings.down,
        OnKeyAction::StdControllerLeft => &mut bindings.left,
        OnKeyAction::StdControllerRight => &mut bindings.right,
        OnKeyAction::StdControllerAButton => &mut bindings.a,
        OnKeyAction::StdControllerBButton => &mut bindings.b,
        OnKeyAction::StdControllerStartButton => &mut bindings.start,
        OnKeyAction::StdControllerSelectButton => &mut bindings.select,
        _ => unreachable!(),
    }
}

fn render_action_binding_group(
    ui: &mut egui::Ui,
    title: &str,
    id: &'static str,
    bindings: &mut BTreeMap<OnKeyAction, Binding>,
    actions: &[OnKeyAction],
    label_column_width: f32,
) -> bool {
    let mut changed = false;

    ui.collapsing(title, |ui| {
        egui::Grid::new(id)
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for action in actions {
                    let binding = bindings.entry(*action).or_insert_with(|| Binding {
                        variant: BindVariant::Unbound,
                        modifiers: egui::Modifiers::NONE,
                        action: *action,
                    });

                    ui.add_sized(
                        [label_column_width, ui.spacing().interact_size.y],
                        egui::Label::new(action.get_display_name()),
                    );
                    changed |= ui.add(Hotkey::with_id(binding, action)).changed();
                    ui.end_row();
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
