use crate::tests::cpu::Cpu;

#[test]
fn test_rol_complete() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0100_0000;

    cpu.mem_write(0x0, 0x2A);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b1000_0000);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x1, 0x26);
    cpu.mem_write(0x2, 0x20);
    cpu.mem_write(0x20, 0b1000_0000);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x20), 0);
    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0100_0000;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x3, 0x2A);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b1000_0001);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x4, 0x26);
    cpu.mem_write(0x5, 0x30);
    cpu.mem_write(0x30, 0b1000_0001);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x31), 0);
    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.x_register = 0x01;

    cpu.mem_write(0x6, 0x36);
    cpu.mem_write(0x7, 0x40);
    cpu.mem_write(0x41, 0b0000_0001);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x41), 0b0000_0010);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x8, 0x2E);
    cpu.mem_write_u16(0x9, 0x1234);
    cpu.mem_write(0x1234, 0b0000_0001);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x1234), 0b0000_0010);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;
    cpu.x_register = 0x01;

    cpu.mem_write(0xB, 0x3E);
    cpu.mem_write_u16(0xC, 0x8000);
    cpu.mem_write(0x8001, 0b1000_0000);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x8001), 0);
    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;
    cpu.accumulator = 0b0000_1111;

    cpu.mem_write(0xE, 0x2A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;
    cpu.accumulator = 0b0000_0000;

    cpu.mem_write(0xF, 0x2A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.accumulator = 0b0100_1111;
    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x10, 0x2A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    cpu.accumulator = 0b1000_1111;
    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x11, 0x2A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    cpu.accumulator = 0b1000_0000;
    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x12, 0x2A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.accumulator = 0b1100_1111;
    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x13, 0x2A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());

    cpu.accumulator = 0b1000_0000;
    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x14, 0x2A);
    cpu.mem_write(0x15, 0x2A);
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0);
    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());

    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 1);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
}

#[test]
fn test_rol_accumulator() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0100_0000;

    cpu.mem_write(0x0, 0x2A);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b1000_0000);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_rol_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x26);
    cpu.mem_write(0x1, 0x20);
    cpu.mem_write(0x20, 0b1000_0000);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x20), 0);
    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_rol_accumulator_with_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0100_0000;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0x2A);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b1000_0001);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_rol_zero_page_with_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0x26);
    cpu.mem_write(0x1, 0x20);
    cpu.mem_write(0x20, 0b1000_0001);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x21), 0);
    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_rol_zero_page_x() {
    let mut cpu = Cpu::test_instance();
    cpu.x_register = 0x01;

    cpu.mem_write(0x0, 0x36);
    cpu.mem_write(0x1, 0x30);
    cpu.mem_write(0x31, 0b0000_0001);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x31), 0b0000_0010);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_rol_absolute() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x2E);
    cpu.mem_write_u16(0x1, 0x1234);
    cpu.mem_write(0x1234, 0b0000_0001);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x1234), 0b0000_0010);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_rol_absolute_x() {
    let mut cpu = Cpu::test_instance();
    cpu.x_register = 0x01;

    cpu.mem_write(0x0, 0x3E);
    cpu.mem_write_u16(0x1, 0x8000);
    cpu.mem_write(0x8001, 0b1000_0000);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x8001), 0);
    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_rol_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_1111;

    cpu.mem_write(0x0, 0x2A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_rol_flags_only_zero_when_only_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0000;

    cpu.mem_write(0x0, 0x2A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_rol_flags_only_negative_when_only_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0100_1111;

    cpu.mem_write(0x0, 0x2A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_rol_flags_only_carry_when_only_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b1000_1111;

    cpu.mem_write(0x0, 0x2A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
}

#[test]
fn test_rol_flags_carry_and_zero_when_carry_and_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b1000_0000;

    cpu.mem_write(0x0, 0x2A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_rol_flags_carry_and_only_negative_when_carry_and_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b1100_1111;

    cpu.mem_write(0x0, 0x2A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_rol_preserves_overflow_flag() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0001;
    cpu.processor_status |= 0b0100_0000; // set V

    cpu.mem_write(0x0, 0x2A); // ROL A
    cpu.step();
    cpu.step();

    assert!(cpu.get_overflow_flag()); // should be unchanged
}

#[test]
fn test_rol_preserves_decimal_flag() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0001;
    cpu.processor_status |= 0b0000_1000; // set D

    cpu.mem_write(0x0, 0x2A); // ROL A
    cpu.step();
    cpu.step();

    assert!(cpu.get_decimal_flag()); // should be unchanged
}

#[test]
fn test_rol_preserves_interrupt_disable_flag() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0001;
    cpu.processor_status |= 0b0000_0100; // set I

    cpu.mem_write(0x0, 0x2A); // ROL A
    cpu.step();
    cpu.step();

    assert!(cpu.get_interrupt_disable_flag()); // should be unchanged
}

#[test]
fn test_rol_preserves_break_flag() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0001;
    cpu.processor_status |= 0b0001_0000; // set B

    cpu.mem_write(0x0, 0x2A); // ROL A
    cpu.step();
    cpu.step();

    assert!(cpu.get_break_flag()); // should be unchanged
}

#[test]
fn test_rol_preserves_unused_flag() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0001;
    cpu.processor_status |= 0b0010_0000; // set U (unused bit 5)

    cpu.mem_write(0x0, 0x2A); // ROL A
    cpu.step();
    cpu.step();

    assert!(cpu.get_unused_flag()); // should be unchanged
}
