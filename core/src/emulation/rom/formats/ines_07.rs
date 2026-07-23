use crate::emulation::rom::{ParseError, RomBuilder, RomFile, RomParser};

#[derive(Debug)]
pub struct Ines07;

impl RomParser for Ines07 {
    fn get_name(&self) -> &'static str { "iNES 0.7" }

    fn parse(&self, rom: &[u8], name: Option<&String>) -> Result<RomFile, ParseError> {
        let prg_rom_size = u32::from(rom[4]) * 16 * 1024;
        let chr_rom_size = u32::from(rom[5]) * 8 * 1024;

        let alternative_nametables = rom[6] & 0b0000_1000 != 0;
        let trainer_present = rom[6] & 0b0000_0100 != 0;
        let is_battery_backed = rom[6] & 0b0000_0010 != 0;
        let hard_wired_nametable_layout = rom[6] & 0b000_0001 != 0;

        let mapper_number = u16::from(rom[6] >> 4) | u16::from(rom[7] & 0xF0);

        Ok(RomBuilder::default()
            .prg_rom_size(prg_rom_size)
            .chr_rom_size(chr_rom_size)
            .mapper_number(mapper_number)
            .alternative_nametables(alternative_nametables)
            .trainer_present(trainer_present)
            .battery_backed(is_battery_backed)
            .hardwired_nametable_layout(hard_wired_nametable_layout)
            .name(name.cloned())
            .build())
    }
}
