use crate::emulation::rom::{ParseError, RomBuilder, RomFile, RomParser};

#[derive(Debug)]
pub struct Ines;

impl RomParser for Ines {
    fn parse(&self, rom: &[u8], name: Option<&String>) -> Result<RomFile, ParseError> {
        let prg_rom_size = rom[4] as u32 * 16 * 1024;
        let chr_rom_size = rom[5] as u32 * 8 * 1024;

        let alternative_nametables = rom[6] & 0b00001000 != 0;
        let trainer_present = rom[6] & 0b00000100 != 0;
        let is_battery_backed = rom[6] & 0b00000010 != 0;
        let hard_wired_nametable_layout = rom[6] & 0b0000001 != 0;

        let mapper_number = (rom[6] >> 4) as u16 | (rom[7] & 0xF0) as u16;
        let playchoice_10_data = rom[7] & 0x2 != 0;
        let vs_unisystem = rom[7] & 0x1 != 0;
        let prg_ram_size = if rom[8] == 0 {
            8 * 1024
        } else {
            rom[8] as u32 * 8 * 1024
        };

        let tv_system = rom[9] & 0x1;

        let console_type = if playchoice_10_data {
            2
        } else if vs_unisystem {
            1
        } else {
            0
        };

        Ok(RomBuilder::default()
            .prg_rom_size(prg_rom_size)
            .prg_ram_size(prg_ram_size)
            .chr_rom_size(chr_rom_size)
            .mapper_number(mapper_number)
            .alternative_nametables(alternative_nametables)
            .trainer_present(trainer_present)
            .battery_backed(is_battery_backed)
            .hardwired_nametable_layout(hard_wired_nametable_layout)
            .console_type(console_type)
            .cpu_ppu_timing(tv_system)
            .name(name.cloned())
            .build())
    }
}
