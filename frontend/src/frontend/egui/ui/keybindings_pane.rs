//! Keybindings pane rendering

use strum::IntoEnumIterator;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::keybindings::{Hotkey, KeybindCategory};

#[derive(Clone, Copy)]
struct SharedLabelWidthCache {
    pixels_per_point: f32,
    key_count: usize,
    width: f32,
}

fn get_shared_label_column_width(ui: &mut egui::Ui, config: &AppConfig) -> f32 {
    const PIXELS_PER_POINT_CACHE_TOLERANCE: f32 = 0.01;

    let cache_id = egui::Id::new("keybindings_shared_label_column_width");
    let pixels_per_point = ui.ctx().pixels_per_point();
    let key_count = config.keybindings.keybindings.len();

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
    let width = config
        .keybindings
        .keybindings
        .keys()
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
    let shared_label_column_width = get_shared_label_column_width(ui, config);

    egui::ScrollArea::vertical().show(ui, |ui| {
        for category in KeybindCategory::iter() {
            ui.collapsing(category.get_name(), |ui| {
                egui::Grid::new(category)
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for (action, binding) in config
                            .keybindings
                            .keybindings
                            .iter_mut()
                            .filter(|(action, _)| action.get_category() == category)
                        {
                            ui.add_sized(
                                [shared_label_column_width, ui.spacing().interact_size.y],
                                egui::Label::new(action.get_display_name()),
                            );
                            changed |= ui.add(Hotkey::with_id(binding, action)).changed();
                            ui.end_row()
                        }
                    });
            });
            ui.separator();
        }

        changed |= render_reset_button(ui, config);
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
