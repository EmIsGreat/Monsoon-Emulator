use crate::emulation::nes::{Nes, RunOptions};

#[test]
fn test_13_rts() {
    let mut emu = Nes::default();
    emu.load_rom((
        &String::from("./tests/nes-test-roms/instr_test-v5/rom_singles/13-rts.nes"),
        false,
    ));
    emu.reset();
    emu.run_until(6977013, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x6018));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0x00, 0xDE, 0xB0, 0x61, 0x0A, 0x31, 0x33, 0x2D, 0x72, 0x74, 0x73, 0x0A, 0x0A, 0x50, 0x61,
        0x73, 0x73, 0x65, 0x64, 0x0A, 0x00,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}
