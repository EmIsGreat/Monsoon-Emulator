use crate::emulation::mem::Memory;
use crate::tests::cpu::Cpu;

#[test]
fn test_brk_vector() {
    let mut cpu = Cpu::new();

    // Create and initialize new Rom
    let mut rom = Memory::new(0x4000, false);

    // Set reset vector to $8000
    rom.init(0x3FFC, 0x00);
    rom.init(0x3FFD, 0x80);
    // Set break handler vector to $8020
    rom.init(0x3FFE, 0x20);
    rom.init(0x3FFF, 0x80);

    // Load 0x20 to acc from $8020
    rom.init(0x20, 0xA9);
    rom.init(0x21, 0x20);

    // Trigger Break at CPU start ($8000)
    cpu.mem_write(0x0, 0x0);

    // Attach ROM mapper
    cpu.attach_test_rom(rom);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0x20)
}
