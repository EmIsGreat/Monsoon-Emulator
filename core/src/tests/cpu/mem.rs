use crate::emulation::mem::Memory;
use crate::tests::cpu::Cpu;

#[test]
fn test_cpu_ram_writeable_and_readable() {
    let mut cpu = Cpu::new();
    cpu.mem_write(0x10, 0x20);
    assert_eq!(cpu.mem_read(0x10), 0x20)
}

#[test]
fn test_cpu_ram_mirroring() {
    let mut cpu = Cpu::new();
    cpu.mem_write(0x10, 0x10);
    assert_eq!(cpu.mem_read(0x810), 0x10);

    cpu.mem_write(0x820, 0x20);
    assert_eq!(cpu.mem_read(0x20), 0x20);

    cpu.mem_write(0x830, 0x30);
    assert_eq!(cpu.mem_read(0x830), 0x30);
}

#[test]
fn test_rom_readable() {
    let mut cpu = Cpu::new();

    // Create and initialize new Rom
    let mut rom = Memory::new(0x4000, false);
    rom.init(0x30, 0x20);
    cpu.attach_test_rom(rom);

    assert_eq!(cpu.mem_read(0x8030), 0x20);
}

#[test]
fn test_rom_non_writeable() {
    let mut cpu = Cpu::new();

    cpu.attach_test_rom(Memory::new(0x4000, false));

    cpu.mem_write(0x8030, 0x20);

    assert_eq!(cpu.mem_read(0x8030), 0x00);
}
