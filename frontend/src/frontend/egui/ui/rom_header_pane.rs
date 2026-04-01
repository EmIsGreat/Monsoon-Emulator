use crate::frontend::egui::config::AppConfig;

fn format_bytes_human_readable(bytes: u32) -> String {
    const UNITS: [&str; 3] = ["Bytes", "KB", "MB"];

    let mut value = bytes as f64;
    let mut unit_idx = 0usize;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{bytes} {}", UNITS[unit_idx])
    } else {
        format!("{value:.2} {} ({bytes} Bytes)", UNITS[unit_idx])
    }
}

pub fn render_rom_header(ui: &mut egui::Ui, config: &AppConfig) {
    if let Some((rom, loaded_rom)) = &config.console_config.loaded_rom {
        egui::Grid::new("rom_header_info")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Filename");
                ui.label(&loaded_rom.name);
                ui.end_row();

                ui.label("Mapper");
                ui.label(rom.mapper.to_string());
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
                    "Vertical"
                } else {
                    "Horizontal"
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
                ui.label(rom.default_expansion_device.to_string());
                ui.end_row();

                ui.label("Misc ROM Count");
                ui.label(rom.misc_rom_count.to_string());
                ui.end_row();

                ui.label("Extended Console Type");
                ui.label(
                    rom.extended_console_type
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "(none)".to_string()),
                );
                ui.end_row();

                ui.label("VS System Hardware Type");
                ui.label(
                    rom.vs_system_hardware_type
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "(none)".to_string()),
                );
                ui.end_row();

                ui.label("VS System PPU Type");
                ui.label(
                    rom.vs_system_ppu_type
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "(none)".to_string()),
                );
                ui.end_row();
            });
    } else {
        ui.label("No ROM loaded.");
    }
}
