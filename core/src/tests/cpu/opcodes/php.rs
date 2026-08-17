use crate::tests::cpu::Cpu;

#[test]
fn test_php_implied_simple() {
    let mut cpu = Cpu::test_instance();
    // Load 0 to acc to trigger Zero flag
    cpu.mem_write(0x0, 0xA9);
    cpu.mem_write(0x1, 0x00);

    // Write Processor Status to Stack
    cpu.mem_write(0x2, 0x08);

    cpu.step();
    cpu.step();

    cpu.step();
    cpu.step();
    cpu.step();

    // Validate that saved Processor Status has Zero flag set
    assert_eq!(cpu.mem_read(0x01FD), 0b0011_0110);
    // Validate that Stack Pointer has been incremented
    assert_eq!(cpu.stack_pointer, 0xFC);
}

#[test]
fn test_php_implied_complicated() {
    let mut cpu = Cpu::test_instance();
    // Load 0 to acc to trigger Zero flag
    cpu.mem_write(0x0, 0xA9);
    cpu.mem_write(0x1, 0x00);

    // Write Processor Status to Stack
    cpu.mem_write(0x2, 0x08);

    cpu.step();
    cpu.step();

    cpu.step();
    cpu.step();
    cpu.step();

    // Load 0x80 to acc to trigger Negative flag
    cpu.mem_write(0x3, 0xA9);
    cpu.mem_write(0x4, 0x80);

    // Write Processor Status to Stack
    cpu.mem_write(0x5, 0x08);

    cpu.step();
    cpu.step();

    cpu.step();
    cpu.step();
    cpu.step();

    // Load 0x66 to acc to reset flags
    cpu.mem_write(0x6, 0xA9);
    cpu.mem_write(0x7, 0x66);

    // Write Processor Status to Stack
    cpu.mem_write(0x8, 0x08);

    cpu.step();
    cpu.step();

    cpu.step();
    cpu.step();
    cpu.step();

    // Validate that saved Processor Status has Zero flag set
    assert_eq!(cpu.mem_read(0x01FD), 0b0011_0110);
    // Validate that saved Processor Status has Negative flag set
    assert_eq!(cpu.mem_read(0x01FC), 0b1011_0100);
    // Validate that saved Processor Status has no flags set
    assert_eq!(cpu.mem_read(0x01FB), 0b0011_0100);
    // Validate that Stack Pointer has been incremented correctly
    assert_eq!(cpu.stack_pointer, 0xFA);
}
