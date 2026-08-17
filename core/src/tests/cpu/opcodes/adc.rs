use crate::tests::cpu::Cpu;

#[test]
fn test_adc_immediate() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x11;
    cpu.mem_write(0x0, 0x69);
    cpu.mem_write(0x1, 0x55);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_adc_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x11;
    cpu.mem_write(0x0, 0x65);
    cpu.mem_write(0x1, 0x33);
    cpu.mem_write(0x33, 0x55);

    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_adc_zero_page_x() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x11;
    cpu.x_register = 0x10;
    cpu.mem_write(0x0, 0x75);
    cpu.mem_write(0x1, 0x33);
    cpu.mem_write(0x43, 0x55);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_adc_absolute() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x11;
    cpu.mem_write(0x0, 0x6D);
    cpu.mem_write_u16(0x1, 0x8333);
    cpu.mem_write(0x8333, 0x55);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_adc_absolute_x() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x11;
    cpu.x_register = 0x10;
    cpu.mem_write(0x0, 0x7D);
    cpu.mem_write_u16(0x1, 0x83FF);
    cpu.mem_write(0x840F, 0x55);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_adc_absolute_y() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x11;
    cpu.y_register = 0x10;
    cpu.mem_write(0x0, 0x79);
    cpu.mem_write_u16(0x1, 0x8333);
    cpu.mem_write(0x8343, 0x55);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_adc_indirect_x() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x11;
    cpu.x_register = 0x10;
    cpu.mem_write(0x0, 0x61);
    cpu.mem_write(0x1, 0x33);
    cpu.mem_write_u16(0x43, 0x8343);
    cpu.mem_write(0x8343, 0x55);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_adc_indirect_y() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x11;
    cpu.y_register = 0x10;
    cpu.mem_write(0x0, 0x71);
    cpu.mem_write(0x1, 0x33);
    cpu.mem_write_u16(0x33, 0x8333);
    cpu.mem_write(0x8343, 0x55);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_adc_with_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x10;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0x69);
    cpu.mem_write(0x1, 0x55);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_adc_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x10;
    cpu.processor_status |= 0b0000_0001;

    cpu.mem_write(0x0, 0x69);
    cpu.mem_write(0x1, 0x55);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
    assert!(!cpu.get_overflow_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_carry_flag());
}

#[test]
fn test_adc_flags_negative_when_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x10;

    cpu.mem_write(0x0, 0x69);
    cpu.mem_write(0x1, 0x80);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x90);
    assert!(!cpu.get_overflow_flag());
    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_carry_flag());
}

#[test]
fn test_adc_flags_carry_when_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0xFF;

    cpu.mem_write(0x0, 0x69);
    cpu.mem_write(0x1, 0x3);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x2);
    assert!(!cpu.get_overflow_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_carry_flag());
}

#[test]
fn test_adc_flags_zero_and_carry_when_zero_and_carry() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0xFF;

    cpu.mem_write(0x0, 0x69);
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
fn test_adc_flags_overflow_when_overflow() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x7F;

    cpu.mem_write(0x0, 0x69);
    cpu.mem_write(0x1, 0x3);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x82);
    assert!(cpu.get_overflow_flag());
    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_carry_flag());
}
