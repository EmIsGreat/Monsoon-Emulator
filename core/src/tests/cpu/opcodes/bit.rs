use crate::tests::cpu::Cpu;

#[test]
fn test_bit_complete() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b1111_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());

    cpu.processor_status = 0b0000_0000;

    cpu.mem_write(0x2, 0x2C);
    cpu.mem_write_u16(0x3, 0x8000);
    cpu.mem_write(0x8000, 0b1111_1111);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());

    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0000_0001;
    cpu.mem_write(0x5, 0x24);
    cpu.mem_write(0x6, 0x0020);
    cpu.mem_write(0x0020, 0b0000_0001);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_overflow_flag());

    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0x7, 0x24);
    cpu.mem_write(0x8, 0x0020);
    cpu.mem_write(0x0020, 0b0001_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_overflow_flag());

    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0100_0001;
    cpu.mem_write(0x9, 0x24);
    cpu.mem_write(0xA, 0x0020);
    cpu.mem_write(0x0020, 0b0101_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());

    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0100_0000;
    cpu.mem_write(0xB, 0x24);
    cpu.mem_write(0xC, 0x0020);
    cpu.mem_write(0x0020, 0b1101_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());

    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0xD, 0x24);
    cpu.mem_write(0xE, 0x0020);
    cpu.mem_write(0x0020, 0b0101_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());

    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0xF, 0x24);
    cpu.mem_write(0x10, 0x0020);
    cpu.mem_write(0x0020, 0b1001_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_overflow_flag());

    cpu.processor_status = 0b0000_0000;

    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0x11, 0x24);
    cpu.mem_write(0x12, 0x0020);
    cpu.mem_write(0x0020, 0b1001_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_overflow_flag());
}

#[test]
fn test_bit_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b1111_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());
}

#[test]
fn test_bit_absolute() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0x0, 0x2C);
    cpu.mem_write_u16(0x1, 0x8000);
    cpu.mem_write(0x8000, 0b1111_1111);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());
}

#[test]
fn test_bit_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0001;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b0000_0001);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_overflow_flag());
}

#[test]
fn test_bit_flags_zero_when_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b0001_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    assert!(!cpu.get_overflow_flag());
}

#[test]
fn test_bit_flags_negative_when_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0010;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b1001_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_overflow_flag());
}

#[test]
fn test_bit_flags_overflow_when_overflow() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0100_0000;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b0101_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());
}

#[test]
fn test_bit_flags_negative_and_overflow_when_negative_and_overflow() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0100_0000;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b1101_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());
}

#[test]
fn test_bit_flags_zero_and_overflow_when_zero_and_overflow() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b0101_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());
}

#[test]
fn test_bit_flags_zero_and_negative_when_zero_and_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b1001_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_overflow_flag());
}

#[test]
fn test_bit_flags_zero_and_negative_and_overflow_when_zero_and_negative_and_overflow() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0b0000_0000;
    cpu.mem_write(0x0, 0x24);
    cpu.mem_write(0x1, 0x0020);
    cpu.mem_write(0x0020, 0b1101_1111);

    cpu.step();
    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
    assert!(cpu.get_overflow_flag());
}
