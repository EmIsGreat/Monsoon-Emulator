use crate::emulation::rom::{ParseError, RomBuilder, RomFile, RomParser};

#[derive(Debug)]
pub struct Ines2;

impl RomParser for Ines2 {
    fn parse(&self, rom: &[u8], name: Option<&String>) -> Result<RomFile, ParseError> {
        let prg_rom_size_lsb = rom[4] as u16;
        let prg_rom_size_msb = (rom[9] & 0xF) as u16;

        let prg_rom_size = Self::get_prg_rom_size(prg_rom_size_lsb, prg_rom_size_msb);

        let chr_rom_size_lsb = rom[5] as u16;
        let chr_rom_size_msb = (rom[9] & 0xF0) as u16;

        let chr_rom_size = Self::get_chr_rom_size(chr_rom_size_lsb, chr_rom_size_msb);

        let mapper_number =
            (rom[6] >> 4) as u16 | (rom[7] & 0xF0) as u16 | ((rom[8] & 0xF) as u16) << 8;

        let submapper_number = (rom[8] & 0xF0) >> 4;
        let alternative_nametables = rom[6] & 0b00001000 != 0;
        let trainer_present = rom[6] & 0b00000100 != 0;
        let battery_backed = rom[6] & 0b00000010 != 0;
        let hard_wired_nametable_layout = rom[6] & 1 != 0;

        let console_type = rom[7] & 0b00000011;

        let prg_ram_size_shifts = rom[10] & 0xF;
        let prg_ram_size = if prg_ram_size_shifts == 0 {
            0u32
        } else {
            64 << prg_ram_size_shifts
        };

        let prg_nvram_shifts = (rom[10] & 0xF0) >> 4;
        let prg_nvram_size = if prg_nvram_shifts == 0 {
            0u32
        } else {
            64 << prg_nvram_shifts
        };

        let chr_ram_shifts = (rom[11] & 0xF) >> 4;
        let chr_ram_size = if chr_ram_shifts == 0 {
            0u32
        } else {
            64 << chr_ram_shifts
        };

        let chr_nvram_shifts = (rom[11] & 0xF0) >> 4;
        let chr_nvram_size = if chr_nvram_shifts == 0 {
            0u32
        } else {
            64 << chr_nvram_shifts
        };

        let cpu_ppu_timing = rom[12] & 0x3;

        let vs_system_ppu_type = if console_type == 1 {
            Some(rom[13] & 0xF)
        } else {
            None
        };

        let vs_system_hardware_type = if console_type == 1 {
            Some(rom[13] & 0xF0)
        } else {
            None
        };

        let extended_console_type = if console_type == 3 {
            Some(rom[13] & 0xF)
        } else {
            None
        };

        let misc_rom_count = rom[14] & 0x3;
        let default_expansion_device = rom[15] & 0b00111111;

        if prg_rom_size as usize + chr_rom_size as usize > rom.len() {
            return Err(ParseError::SizeBiggerThanFile);
        }

        Ok(RomBuilder::default()
            .prg_rom_size(prg_rom_size)
            .prg_ram_size(prg_ram_size)
            .prg_nvram_size(prg_nvram_size)
            .chr_rom_size(chr_rom_size)
            .chr_ram_size(chr_ram_size)
            .chr_nvram_size(chr_nvram_size)
            .mapper_number(mapper_number)
            .submapper_number(submapper_number)
            .alternative_nametables(alternative_nametables)
            .trainer_present(trainer_present)
            .battery_backed(battery_backed)
            .hardwired_nametable_layout(hard_wired_nametable_layout)
            .cpu_ppu_timing(cpu_ppu_timing)
            .vs_system_ppu_type(vs_system_ppu_type)
            .vs_system_hardware_type(vs_system_hardware_type)
            .extended_console_type(extended_console_type)
            .misc_rom_count(misc_rom_count)
            .default_expansion_device(default_expansion_device)
            .name(name.cloned())
            .build())
    }
}

impl Ines2 {
    pub fn get_prg_rom_size(prg_rom_size_lsb: u16, prg_rom_size_msb: u16) -> u32 {
        if prg_rom_size_msb <= 0xE {
            (((prg_rom_size_msb << 8) | prg_rom_size_lsb) as u32) * 16 * 1024
        } else {
            let exponent = (prg_rom_size_lsb >> 2) as u32;
            let mult = ((prg_rom_size_lsb & 0b11) as u32 * 2) + 1;
            (1 << exponent) * mult
        }
    }

    pub fn get_chr_rom_size(chr_rom_size_lsb: u16, chr_rom_size_msb: u16) -> u32 {
        if chr_rom_size_msb <= 0xE {
            (((chr_rom_size_msb << 8) | chr_rom_size_lsb) as u32) * 8 * 1024
        } else {
            let exponent = (chr_rom_size_lsb >> 2) as u32;
            let mult = ((chr_rom_size_lsb & 0b11) as u32 * 2) + 1;
            (1 << exponent) * mult
        }
    }
}
