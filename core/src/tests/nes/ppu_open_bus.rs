use crate::emulation::nes::{Nes, RunOptions};

#[test]
fn test_ppu_open_bus() {
    let mut emu = Nes::default();
    emu.load_rom((
        &String::from("./tests/nes-test-roms/ppu_open_bus/ppu_open_bus.nes"),
        false,
    ));
    emu.power();

    emu.run_until(100_000_000, RunOptions::default())
        .expect("Error while running test");

    let whole_mem = emu.get_memory_debug(Some(0x6000..=0x601A));
    let cpu_mem = whole_mem[0].as_slice();

    let expected = [
        0x00, 0xDE, 0xB0, 0x61, 0x0A, 0x70, 0x70, 0x75, 0x5F, 0x6F, 0x70, 0x65, 0x6E, 0x5F, 0x62,
        0x75, 0x73, 0x0A, 0x0A, 0x50, 0x61, 0x73, 0x73, 0x65, 0x64, 0x0A, 0x00,
    ];

    assert_eq!(cpu_mem[0], 0);
    assert_eq!(&cpu_mem[..expected.len()], &expected);
}
