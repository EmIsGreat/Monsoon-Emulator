use crate::emulation::rom::{ParseError, RomBuilder, RomFile, RomParser};

#[derive(Debug)]
pub struct Ines;

impl RomParser for Ines {
    fn get_name(&self) -> &'static str { "iNES" }

    #[allow(clippy::similar_names)]
    fn parse(&self, rom: &[u8], name: Option<&String>) -> Result<RomFile, ParseError> {
        let prg_rom_size = u32::from(rom[4]) * 16 * 1024;
        let chr_rom_size = u32::from(rom[5]) * 8 * 1024;

        let alternative_nametables = rom[6] & 0b0000_1000 != 0;
        let trainer_present = rom[6] & 0b0000_0100 != 0;
        let is_battery_backed = rom[6] & 0b0000_0010 != 0;
        let hard_wired_nametable_layout = rom[6] & 0b000_0001 != 0;

        let mapper_number = u16::from(rom[6] >> 4) | u16::from(rom[7] & 0xF0);
        let playchoice_10_data = rom[7] & 0x2 != 0;
        let vs_unisystem = rom[7] & 0x1 != 0;
        let prg_ram_size = if rom[8] == 0 {
            8 * 1024
        } else {
            u32::from(rom[8]) * 8 * 1024
        };

        let chr_ram_size = if rom[5] == 0 { 8 * 1024 } else { 0 };

        let tv_system = rom[9] & 0x1;

        let console_type = if playchoice_10_data {
            2
        } else {
            u8::from(vs_unisystem)
        };

        Ok(RomBuilder::default()
            .prg_rom_size(prg_rom_size)
            .prg_ram_size(prg_ram_size)
            .chr_rom_size(chr_rom_size)
            .chr_ram_size(chr_ram_size)
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
