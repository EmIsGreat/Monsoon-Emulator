use crate::tests::cpu::Cpu;

#[test]
fn test_pha_implied_simple() {
    let mut cpu = Cpu::test_instance();
    // Init acc
    cpu.accumulator = 0x66;
    // Write acc to stack
    cpu.mem_write(0x0, 0x48);

    cpu.step();
    cpu.step();
    cpu.step();

    // Validate that acc was saved to stack
    assert_eq!(cpu.mem_read(0x01FD), 0x66);
    // Validate that stack pointer was incremented
    assert_eq!(cpu.stack_pointer, 0xFC);
}

#[test]
fn test_pha_implied_complicated() {
    let mut cpu = Cpu::test_instance();
    // Init acc
    cpu.accumulator = 0x66;
    // Write acc to stack
    cpu.mem_write(0x0, 0x48);

    cpu.step();
    cpu.step();
    cpu.step();

    // Init acc
    cpu.accumulator = 0x88;
    // Write acc to stack
    cpu.mem_write(0x1, 0x48);

    cpu.step();
    cpu.step();
    cpu.step();

    // Init acc
    cpu.accumulator = 0x99;
    // Write acc to stack
    cpu.mem_write(0x2, 0x48);

    cpu.step();
    cpu.step();
    cpu.step();

    // Validate that acc was saved to correct address
    assert_eq!(cpu.mem_read(0x01FD), 0x66);
    // Validate that acc was saved to correct address
    assert_eq!(cpu.mem_read(0x01FC), 0x88);
    // Validate that acc was saved to correct address
    assert_eq!(cpu.mem_read(0x01FB), 0x99);
    // Validate that stack pointer was incremented
    assert_eq!(cpu.stack_pointer, 0xFA);
}
