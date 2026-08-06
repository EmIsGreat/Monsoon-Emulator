use egui::RichText;
use monsoon_core::util::format_bytes_human_readable;

use crate::frontend::egui::config::AppConfig;
use crate::frontend::egui::ui::widgets::wrapping_label;

pub fn render_rom_header(ui: &mut egui::Ui, config: &AppConfig) {
    if let Some((rom, loaded_rom)) = &config.console_config.loaded_rom {
        egui::Grid::new("rom_header_info")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Filename");
                wrapping_label(ui, &loaded_rom.name, 3);
                ui.end_row();

                ui.label("Mapper");
                wrapping_label(ui, &rom.mapper.to_string(), 3);
                ui.end_row();

                ui.label("Submapper");
                ui.label(rom.submapper_number.to_string());
                ui.end_row();

                ui.label("CPU/PPU Timing");
                ui.label(rom.timing_region.to_string());
                ui.end_row();

                ui.label("Console Type");
                ui.label(rom.console_type.to_string());
                ui.end_row();

                ui.label("PRG ROM Size");
                ui.label(format_bytes_human_readable(rom.prg_memory.prg_rom_size));
                ui.end_row();

                ui.label("PRG RAM Size");
                ui.label(format_bytes_human_readable(rom.prg_memory.prg_ram_size));
                ui.end_row();

                ui.label("PRG NVRAM Size");
                ui.label(format_bytes_human_readable(rom.prg_memory.prg_nvram_size));
                ui.end_row();

                ui.label("CHR ROM Size");
                ui.label(format_bytes_human_readable(rom.chr_memory.chr_rom_size));
                ui.end_row();

                ui.label("CHR RAM Size");
                ui.label(format_bytes_human_readable(rom.chr_memory.chr_ram_size));
                ui.end_row();

                ui.label("CHR NVRAM Size");
                ui.label(format_bytes_human_readable(rom.chr_memory.chr_nvram_size));
                ui.end_row();

                ui.label("Hardwired Nametable Layout");
                ui.label(if rom.hardwired_nametable_layout {
                    "Horizontal"
                } else {
                    "Vertical/Mapper Controlled"
                });
                ui.end_row();

                ui.label("Battery Backed");
                ui.label(rom.is_battery_backed.to_string());
                ui.end_row();

                ui.label("Trainer Present");
                ui.label(rom.trainer_present.to_string());
                ui.end_row();

                ui.label("Alternative Nametables");
                ui.label(rom.alternative_nametables.to_string());
                ui.end_row();

                ui.label("Default Expansion Device");
                wrapping_label(ui, &rom.default_expansion_device.to_string(), 3);
                ui.end_row();

                ui.label("Misc ROM Count");
                ui.label(rom.misc_rom_count.to_string());
                ui.end_row();

                ui.label("Extended Console Type");
                wrapping_label(ui, &rom.extended_console_type
                    .map_or_else(|| "(none)".to_string(), |v| v.to_string()), 3);
                ui.end_row();

                ui.label("VS System Hardware Type");
                wrapping_label(ui,
                               &rom.vs_system_hardware_type
                                   .map_or_else(|| "(none)".to_string(), |v| v.to_string()),
                               3);
                ui.end_row();

                ui.label("VS System PPU Type");
                ui.label(
                    rom.vs_system_ppu_type
                        .map_or_else(|| "(none)".to_string(), |v| v.to_string()),
                );
                ui.end_row();
                ui.label("File Type");
                ui.label(&rom.format_name);
                ui.end_row();

                ui.label("Raw Header Data");
                let row1 = rom.raw_header_bytes
                    .iter()
                    .take(8)
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");

                let row2 = rom.raw_header_bytes
                    .iter()
                    .skip(8)
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");

                ui.label(RichText::new(format!("{row1}\n{row2}")).monospace());
            });
    } else {
        ui.label("No ROM loaded.");
    }
}
