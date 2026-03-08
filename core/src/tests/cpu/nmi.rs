use crate::emulation::mem::{Memory, MemoryDevice, Rom};
use crate::emulation::nes::Nes;

#[test]
fn test_nmi_vector() {
    let mut nes = Nes::default();
    nes.power();

    // Create and initialize new Rom
    let mut rom = Rom::new(0xBFE0);
    // Set Reset vector to 0x4020
    rom.init(0xBFDC, 0x30);
    rom.init(0xBFDD, 0x40);

    // Set NMI vector to 0x4020
    rom.init(0xBFDA, 0x20);
    rom.init(0xBFDB, 0x40);

    // Load 0x20 to acc from 4020
    rom.init(0x0, 0xA9);
    rom.init(0x1, 0x20);

    rom.init(0x10, 0xA9);
    rom.init(0x11, 0x30);

    // Attach new Rom memory device to cpu
    nes.cpu.memory.add_memory(0x4020..=0xFFFF, Memory::Rom(rom));

    // Manually force an nmi
    nes.ppu.borrow().nmi_requested.set(true);

    nes.reset();

    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();

    nes.cpu.step();
    nes.cpu.step();

    assert_eq!(nes.cpu.accumulator, 0x30);

    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();
    nes.cpu.step();

    nes.cpu.step();
    nes.cpu.step();
    assert_eq!(nes.cpu.accumulator, 0x20)
}
