use crate::emulation::cpu::Cpu;

#[test]
fn test_brk_storing() {
    let mut cpu = Cpu::test_instance();
    // test_instance mocks the reset state for convenience, so I need to un-mock it
    // here because we do an actual reset.
    cpu.stack_pointer = 0x0;
    cpu.processor_status |= 0b00000001;
    cpu.mem_write_u16(0xFFFE, 0xFF00);
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
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read_u16(0x01FC), 0x2);
    assert_eq!(cpu.mem_read(0x01FB), 0b00110101);
}

#[test]
fn test_brk_interrupt_vector() {
    let mut cpu = Cpu::test_instance();
    // test_instance mocks the reset state for convenience, so I need to un-mock it
    // here because we do an actual reset.
    cpu.stack_pointer = 0x0;
    cpu.processor_status |= 0b00000001;
    cpu.mem_write_u16(0xFFFE, 0xF000);
    cpu.mem_write(0xF000, 0xA9);
    cpu.mem_write(0xF001, 0x66);

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
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read_u16(0x01FC), 0x2u16);
    assert_eq!(cpu.mem_read(0x01FB), 0b00110101);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66)
}
