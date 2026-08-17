use crate::tests::cpu::Cpu;

#[test]
fn test_pla_implied_simple() {
    let mut cpu = Cpu::test_instance();
    // Init acc
    cpu.accumulator = 0x66;

    // Write acc to Stack
    cpu.mem_write(0x0, 0x48);

    cpu.step();
    cpu.step();
    cpu.step();

    // Reset acc
    cpu.accumulator = 0x00;

    // Load acc from stack
    cpu.mem_write(0x1, 0x68);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    // Validate that acc has been loaded correctly
    assert_eq!(cpu.accumulator, 0x66);
    // Validate that stack pointer is correct
    assert_eq!(cpu.stack_pointer, 0xFD);
}

#[test]
fn test_pla_implied_complicated() {
    let mut cpu = Cpu::test_instance();
    // Init acc
    cpu.accumulator = 0x0;

    // Write acc to stack
    cpu.mem_write(0x0, 0x48);
    cpu.step();
    cpu.step();
    cpu.step();

    cpu.accumulator = 0x80;

    // Write acc to stack
    cpu.mem_write(0x1, 0x48);
    cpu.step();
    cpu.step();
    cpu.step();

    cpu.accumulator = 0x70;

    // Write acc to stack
    cpu.mem_write(0x2, 0x48);
    cpu.step();
    cpu.step();
    cpu.step();

    // Load acc from stack
    cpu.mem_write(0x3, 0x68);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    // Validate that correct value was loaded
    assert_eq!(cpu.accumulator, 0x70);
    // Validate that correct flags have been set
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    // Load acc from stack
    cpu.mem_write(0x4, 0x68);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    // Validate that correct value was loaded
    assert_eq!(cpu.accumulator, 0x80);
    // Validate that correct flags have been set
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());

    // Load acc from stack
    cpu.mem_write(0x5, 0x68);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    // Validate that correct value was loaded
    assert_eq!(cpu.accumulator, 0x00);
    // Validate that correct flags have been set
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}
