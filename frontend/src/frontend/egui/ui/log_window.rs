use monsoon_core::util::ToBytes;

use crate::channel_emu::ChannelEmulator;
use crate::frontend::egui::config::AppConfig;
use crate::frontend::util::{self, FileType};

/// Wrapper for raw bytes that implements ToBytes for save dialog export.
struct ExportableData(Vec<u8>);

impl ToBytes for ExportableData {
    fn to_bytes(&self, _format: Option<String>) -> Vec<u8> { self.0.clone() }
}

const MAX_VISIBLE_LOG_CHARS: usize = 200_000;

pub fn render_log_viewer(ui: &mut egui::Ui, config: &mut AppConfig, channel_emu: &mut ChannelEmulator) {
    let mut trace_enabled = channel_emu.nes.trace_enabled();
    if ui
        .checkbox(&mut trace_enabled, "Enable CPU trace logging")
        .changed()
    {
        channel_emu.nes.set_trace_enabled(trace_enabled);
    }

    ui.horizontal(|ui| {
        if ui.button("Reset log").clicked() {
            channel_emu.nes.clear_trace_log();
        }

        let has_log = channel_emu
            .nes
            .trace_log()
            .is_some_and(|trace| !trace.log.is_empty());
        if ui
            .add_enabled(has_log, egui::Button::new("Save log to file"))
            .clicked()
            && let Some(trace) = channel_emu.nes.trace_log()
        {
            let exportable = ExportableData(trace.log.clone().into_bytes());
            util::spawn_save_dialog(
                None,
                config.user_config.previous_savestate_save_dir.as_ref(),
                FileType::All,
                Box::new(exportable),
            );
        }
    });

    ui.separator();
    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
        if let Some(trace) = channel_emu.nes.trace_log() {
            let log = &trace.log;
            let start = if log.len() > MAX_VISIBLE_LOG_CHARS {
                let mut idx = log.len() - MAX_VISIBLE_LOG_CHARS;
                while idx < log.len() && !log.is_char_boundary(idx) {
                    idx += 1;
                }
                idx
            } else {
                0
            };
            ui.monospace(&log[start..]);
        } else {
            ui.monospace("");
        }
        });
}
