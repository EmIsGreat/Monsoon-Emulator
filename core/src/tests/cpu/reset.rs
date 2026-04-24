use crate::emulation::mem::Memory;
use crate::tests::cpu::Cpu;

#[test]
fn test_reset_vector() {
    let mut cpu = Cpu::new();

    let mut rom = Memory::new(0x4000, false);
    // Set reset vector to $8000
    rom.init(0x3FFC, 0x00);
    rom.init(0x3FFD, 0x80);
    // LDA #$20 at $8000
    rom.init(0x0000, 0xA9);
    rom.init(0x0001, 0x20);

    cpu.attach_test_rom(rom);

    cpu.reset();
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
