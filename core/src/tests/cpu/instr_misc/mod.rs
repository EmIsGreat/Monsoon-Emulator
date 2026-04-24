use crate::emulation::nes::{Nes, RunOptions};

#[test]
fn test_instr_misc() {
    let mut emu = Nes::default();
    let loaded = emu.load_rom(&String::from(
        "./tests/nes-test-roms/instr_misc/instr_misc.nes",
    ));

    if !loaded.0 {
        eprint!(
            "Mapper of Rom (id: {}) is not implemented... Aborting",
            loaded.1
        );
        return;
    }

    emu.reset();
    emu.run_until(20_000_000, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x6031));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0x00, 0xDE, 0xB0, 0x61, 0x0A, 0x30, 0x32, 0x2D, 0x62, 0x72, 0x61, 0x6E, 0x63, 0x68, 0x5F,
        0x77, 0x72, 0x61, 0x70, 0x0A, 0x0A, 0x50, 0x61, 0x73, 0x73, 0x65, 0x64, 0x0A, 0x41, 0x6C,
        0x6C, 0x20, 0x34, 0x20, 0x74, 0x65, 0x73, 0x74, 0x73, 0x20, 0x70, 0x61, 0x73, 0x73, 0x65,
        0x64, 0x0A, 0x0A, 0x0A, 0x00,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}

#[test]
fn test_instr_misc_01_abs_x_wrap() {
    let mut emu = Nes::default();
    let loaded = emu.load_rom(&String::from(
        "./tests/nes-test-roms/instr_misc/rom_singles/01-abs_x_wrap.nes",
    ));

    if !loaded.0 {
        eprint!(
            "Mapper of Rom (id: {}) is not implemented... Aborting",
            loaded.1
        );
        return;
    }

    emu.reset();
    emu.run_until(20_000_000, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x6031));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0x0, 0xDE, 0xB0, 0x61, 0xA, 0x30, 0x31, 0x2D, 0x61, 0x62, 0x73, 0x5F, 0x78, 0x5F, 0x77,
        0x72, 0x61, 0x70, 0xA, 0xA, 0x50, 0x61, 0x73, 0x73, 0x65, 0x64, 0xA, 0x0,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}

#[test]
fn test_instr_misc_02_branch_wrap() {
    let mut emu = Nes::default();
    let loaded = emu.load_rom(&String::from(
        "./tests/nes-test-roms/instr_misc/rom_singles/02-branch_wrap.nes",
    ));

    if !loaded.0 {
        eprint!(
            "Mapper of Rom (id: {}) is not implemented... Aborting",
            loaded.1
        );
        return;
    }

    emu.reset();
    emu.run_until(20_000_000, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x6031));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0, 0xDE, 0xB0, 0x61, 0xA, 0x30, 0x32, 0x2D, 0x62, 0x72, 0x61, 0x6E, 0x63, 0x68, 0x5F, 0x77,
        0x72, 0x61, 0x70, 0xA, 0xA, 0x50, 0x61, 0x73, 0x73, 0x65, 0x64, 0xA,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}

#[test]
fn test_instr_misc_03_dummy_reads() {
    let mut emu = Nes::default();
    let loaded = emu.load_rom(&String::from(
        "./tests/nes-test-roms/instr_misc/rom_singles/03-dummy_reads.nes",
    ));

    if !loaded.0 {
        eprint!(
            "Mapper of Rom (id: {}) is not implemented... Aborting",
            loaded.1
        );
        return;
    }

    emu.reset();
    emu.run_until(20_500_000, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x6050));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0x00, 0xDE, 0xB0, 0x61, 0x0A, 0x30, 0x33, 0x2D, 0x64, 0x75, 0x6D, 0x6D, 0x79, 0x5F, 0x72,
        0x65, 0x61, 0x64, 0x73, 0x0A, 0x0A, 0x50, 0x61, 0x73, 0x73, 0x65, 0x64, 0x0A, 0x00,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}

#[test]
fn test_instr_misc_04_dummy_reads_apu() {
    let mut emu = Nes::default();
    let loaded = emu.load_rom(&String::from(
        "./tests/nes-test-roms/instr_misc/rom_singles/04-dummy_reads_apu.nes",
    ));

    if !loaded.0 {
        assert!(
            loaded.0,
            "Mapper of Rom (id: {}) is not implemented... Aborting",
            loaded.1
        );
    }

    emu.reset();
    emu.run_until(20_000_000, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x6100));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0, 0xDE, 0xB0, 61, 0xA, 30, 31, 0x2D, 61, 62, 73, 0x5F, 78, 0x5F, 77, 72, 61, 70, 0xA, 0xA,
        50, 61, 73, 73, 65, 64, 0xA, 0,
    ];

    println!("{:02X?}", &cpu_mem);

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}
