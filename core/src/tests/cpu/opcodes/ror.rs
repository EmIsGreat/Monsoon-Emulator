use crate::tests::cpu::Cpu;

#[test]
fn test_ror_complete() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0100_0000;

    cpu.mem_write(0x0, 0x6A);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b0010_0000);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x1, 0x66);
    cpu.mem_write(0x2, 0x20);
    cpu.mem_write(0x20, 0b1000_0000);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0b0100_0000), 0);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0100_0000;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x3, 0x6A);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b1010_0000);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x4, 0x66);
    cpu.mem_write(0x5, 0x20);
    cpu.mem_write(0x20, 0b1000_0001);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x20), 0b1100_0000);
    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.x_register = 0x01;

    cpu.mem_write(0x6, 0x76);
    cpu.mem_write(0x7, 0x30);
    cpu.mem_write(0x31, 0b0100_0000);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x31), 0b0010_0000);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x8, 0x6E);
    cpu.mem_write_u16(0x9, 0x1234);
    cpu.mem_write(0x1234, 0b0000_0010);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x1234), 0b0000_0001);
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

    assert_eq!(cpu.mem_read(0b0100_0000), 0);
    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0001_1110;

    cpu.mem_write(0xE, 0x6A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0000_0000;

    cpu.mem_write(0xF, 0x6A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0000_1111;

    cpu.mem_write(0x10, 0x6A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0000_0001;

    cpu.mem_write(0x11, 0x6A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ror_accumulator() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0100_0000;

    cpu.mem_write(0x0, 0x6A);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b0010_0000);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ror_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x66);
    cpu.mem_write(0x1, 0x20);
    cpu.mem_write(0x20, 0b1000_0000);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x20), 0b0100_0000);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ror_accumulator_with_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0100_0000;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0x6A);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b1010_0000);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_ror_zero_page_with_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0x66);
    cpu.mem_write(0x1, 0x20);
    cpu.mem_write(0x20, 0b1000_0001);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x20), 0b1100_0000);
    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_ror_zero_page_x() {
    let mut cpu = Cpu::test_instance();
    cpu.x_register = 0x01;

    cpu.mem_write(0x0, 0x76);
    cpu.mem_write(0x1, 0x30);
    cpu.mem_write(0x31, 0b0100_0000);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x31), 0b0010_0000);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ror_absolute() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x6E);
    cpu.mem_write_u16(0x1, 0x1234);
    cpu.mem_write(0x1234, 0b0000_0010);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x1234), 0b0000_0001);
    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ror_absolute_x() {
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

    assert_eq!(cpu.mem_read(0b0100_0000), 0);
    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ror_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0001_1110;

    cpu.mem_write(0x0, 0x6A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ror_flags_only_zero_when_only_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0000;

    cpu.mem_write(0x0, 0x6A);

    cpu.step();
    cpu.step();

    assert!(!cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ror_flags_only_carry_when_only_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_1111;

    cpu.mem_write(0x0, 0x6A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
}

#[test]
fn test_ror_flags_carry_and_zero_when_carry_and_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0001;

    cpu.mem_write(0x0, 0x6A);

    cpu.step();
    cpu.step();

    assert!(cpu.get_carry_flag());
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}
