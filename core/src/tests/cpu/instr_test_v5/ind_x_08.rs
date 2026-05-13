use crate::emulation::nes::{Nes, RunOptions};

#[test]
fn test_08_ind_x() {
    let mut emu = Nes::default();
    emu.load_rom((
        &String::from("./tests/nes-test-roms/instr_test-v5/rom_singles/08-ind_x.nes"),
        false,
    ));
    emu.reset();
    emu.run_until(62149589, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x6016));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0x00, 0xDE, 0xB0, 0x61, 0x0A, 0x30, 0x38, 0x2D, 0x69, 0x6E, 0x64, 0x5F, 0x78, 0x0A, 0x0A,
        0x50, 0x61, 0x73, 0x73, 0x65, 0x64, 0x0A, 0x00,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}
