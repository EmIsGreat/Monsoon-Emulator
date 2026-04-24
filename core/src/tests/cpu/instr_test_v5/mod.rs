#[cfg(test)]
mod abs_xy_07;
#[cfg(test)]
mod absolute_06;
#[cfg(test)]
mod basics_01;
#[cfg(test)]
mod branches_10;
#[cfg(test)]
mod brk_15;
#[cfg(test)]
mod immediate_03;
#[cfg(test)]
mod implied_02;
#[cfg(test)]
mod ind_x_08;
#[cfg(test)]
mod ind_y_09;
#[cfg(test)]
mod jmp_jsr_12;
#[cfg(test)]
mod rti_14;
#[cfg(test)]
mod rts_13;
#[cfg(test)]
mod special_16;
#[cfg(test)]
mod stack_11;
#[cfg(test)]
mod zero_page_04;
#[cfg(test)]
mod zp_xy_05;

use crate::emulation::nes::{Nes, RunOptions};

#[test]
fn test_official_only() {
    let mut emu = Nes::default();
    let loaded = emu.load_rom(&String::from(
        "./tests/nes-test-roms/instr_test-v5/official_only.nes",
    ));

    if !loaded.0 {
        assert!(
            loaded.0,
            "Mapper of Rom (id: {}) is not implemented... Aborting",
            loaded.1
        );
    }

    emu.reset();
    emu.run_until(750_000_000, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x601C));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0x00, 0xDE, 0xB0, 0x61, 0x41, 0x6C, 0x6C, 0x20, 0x31, 0x36, 0x20, 0x74, 0x65, 0x73, 0x74,
        0x73, 0x20, 0x70, 0x61, 0x73, 0x73, 0x65, 0x64, 0x0A, 0x0A, 0x0A, 0x00, 0x0A, 0x00,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}

#[test]
fn test_all_instrs() {
    let mut emu = Nes::default();
    let loaded = emu.load_rom(&String::from(
        "./tests/nes-test-roms/instr_test-v5/all_instrs.nes",
    ));

    if !loaded.0 {
        assert!(
            loaded.0,
            "Mapper of Rom (id: {}) is not implemented... Aborting",
            loaded.1
        );
    }

    emu.reset();
    emu.run_until(900_000_000, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x601C));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0x00, 0xDE, 0xB0, 0x61, 0x41, 0x6C, 0x6C, 0x20, 0x31, 0x36, 0x20, 0x74, 0x65, 0x73, 0x74,
        0x73, 0x20, 0x70, 0x61, 0x73, 0x73, 0x65, 0x64, 0x0A, 0x0A, 0x0A, 0x00, 0x0A, 0x00,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}
