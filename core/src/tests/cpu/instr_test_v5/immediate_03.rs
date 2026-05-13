use crate::emulation::nes::{Nes, RunOptions};

#[test]
fn test_03_immediate() {
    let mut emu = Nes::default();
    emu.board.cpu.ane_constant = 0xFF;

    emu.load_rom((
        &String::from("./tests/nes-test-roms/instr_test-v5/rom_singles/03-immediate.nes"),
        false,
    ));

    emu.reset();
    emu.run_until(41064917, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x601A));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0x00, 0xDE, 0xB0, 0x61, 0x0A, 0x30, 0x33, 0x2D, 0x69, 0x6D, 0x6D, 0x65, 0x64, 0x69, 0x61,
        0x74, 0x65, 0x0A, 0x0A, 0x50, 0x61, 0x73, 0x73, 0x65, 0x64, 0x0A, 0x00,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}
