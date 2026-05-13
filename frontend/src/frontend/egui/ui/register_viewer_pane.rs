use monsoon_core::emulation::ppu_util::RegisterMap;

use crate::frontend::egui::textures::EmuTextures;

fn render_register_table(ui: &mut egui::Ui, table_id: &str, registers: &RegisterMap) {
    let mut rows = registers.iter().collect::<Vec<_>>();
    rows.sort_by(|(left, _), (right, _)| left.cmp(right));

    egui::Grid::new(table_id)
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Register");
            ui.strong("Value");
            ui.end_row();

            for (name, entry) in rows {
                ui.label(name);
                ui.monospace(entry.formatted_value());
                ui.end_row();
            }
        });
}

pub fn render_register_viewer(ui: &mut egui::Ui, emu_textures: &EmuTextures) {
    if let Some(register_data) = &emu_textures.register_data {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("PPU Registers");
            render_register_table(ui, "register_viewer_ppu", &register_data.ppu);

            ui.separator();
            ui.heading("APU Registers");
            render_register_table(ui, "register_viewer_apu", &register_data.apu);

            ui.separator();
            ui.heading("Mapper Registers");
            let mut mapper_tables = register_data.mapper.iter().collect::<Vec<_>>();
            mapper_tables.sort_by(|(left, _), (right, _)| left.cmp(right));

            for (table_name, table_registers) in mapper_tables {
                egui::CollapsingHeader::new(table_name.as_str())
                    .default_open(true)
                    .show(ui, |ui| {
                        render_register_table(
                            ui,
                            &format!("register_viewer_mapper_{table_name}"),
                            table_registers,
                        );
                    });
            }
        });
    } else {
        ui.label("Waiting for register data...");
    }
}
