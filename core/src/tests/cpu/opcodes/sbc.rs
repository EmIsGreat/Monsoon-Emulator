use crate::tests::cpu::Cpu;

#[test]
fn test_sbc_immediate() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.mem_write(0x0, 0xE9);
    cpu.mem_write(0x1, 0x11);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x43);
}

#[test]
fn test_sbc_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.mem_write(0x0, 0xE5);
    cpu.mem_write(0x1, 0x33);
    cpu.mem_write(0x33, 0x11);

    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x43);
}

#[test]
fn test_sbc_zero_page_x() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.x_register = 0x10;
    cpu.mem_write(0x0, 0xF5);
    cpu.mem_write(0x1, 0x33);
    cpu.mem_write(0x43, 0x11);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x43);
}

#[test]
fn test_sbc_absolute() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.mem_write(0x0, 0xED);
    cpu.mem_write_u16(0x1, 0x8333);
    cpu.mem_write(0x8333, 0x11);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x43);
}

#[test]
fn test_sbc_absolute_x() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.x_register = 0x10;
    cpu.mem_write(0x0, 0xFD);
    cpu.mem_write_u16(0x1, 0x8333);
    cpu.mem_write(0x8343, 0x11);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x43);
}

#[test]
fn test_sbc_absolute_y() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.y_register = 0x10;
    cpu.mem_write(0x0, 0xF9);
    cpu.mem_write_u16(0x1, 0x8333);
    cpu.mem_write(0x8343, 0x11);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x43);
}

#[test]
fn test_sbc_indirect_x() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.x_register = 0x10;
    cpu.mem_write(0x0, 0xE1);
    cpu.mem_write(0x1, 0x33);
    cpu.mem_write_u16(0x43, 0x8343);
    cpu.mem_write(0x8343, 0x11);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x43);
}

#[test]
fn test_sbc_indirect_y() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.y_register = 0x10;
    cpu.mem_write(0x0, 0xF1);
    cpu.mem_write(0x1, 0x33);
    cpu.mem_write_u16(0x33, 0x8333);
    cpu.mem_write(0x8343, 0x11);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x43);
}

#[test]
fn test_sbc_with_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0xE9);
    cpu.mem_write(0x1, 0x11);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x44);
}

#[test]
fn test_sbc_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x55;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0xE9);
    cpu.mem_write(0x1, 0x11);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x44);
    assert!(!cpu.get_overflow_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_carry_flag());
}

#[test]
fn test_sbc_flags_negative_when_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x90;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0xE9);
    cpu.mem_write(0x1, 0x10);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x80);
    assert!(!cpu.get_overflow_flag());
    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_carry_flag());
}

#[test]
fn test_sbc_flags_carry_and_negative_when_carry_and_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x0;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0xE9);
    cpu.mem_write(0x1, 0x3);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0xFD);
    assert!(!cpu.get_overflow_flag());
    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_carry_flag());
}

#[test]
fn test_sbc_flags_zero_when_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x1;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0xE9);
    cpu.mem_write(0x1, 0x1);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x0);
    assert!(!cpu.get_overflow_flag());
    assert!(!cpu.get_negative_flag());
    assert!(cpu.get_zero_flag());
    assert!(cpu.get_carry_flag());
}

#[test]
fn test_sbc_flags_overflow_when_overflow() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x80;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0xE9);
    cpu.mem_write(0x1, 0x10);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x70);
    assert!(cpu.get_overflow_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_carry_flag());
}
