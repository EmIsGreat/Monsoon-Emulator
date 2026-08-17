use crate::tests::cpu::Cpu;

#[test]
fn test_ora_complete() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x09);
    cpu.mem_write(0x1, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);

    cpu.mem_write(0x2, 0x05);
    cpu.mem_write(0x3, 0x0025);
    cpu.mem_write(0x0025, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);

    cpu.mem_write(0x4, 0x15);
    cpu.mem_write(0x5, 0x0035);
    cpu.x_register = 0x0010;
    cpu.mem_write(0x0045, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);

    cpu.mem_write(0x6, 0x0D);
    cpu.mem_write_u16(0x7, 0x8D2D);
    cpu.mem_write(0x8D2D, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);

    cpu.mem_write(0x9, 0x1D);
    cpu.mem_write_u16(0xA, 0x8D3D);
    cpu.x_register = 0x0010;
    cpu.mem_write(0x8D4D, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);

    cpu.mem_write(0xC, 0x19);
    cpu.mem_write_u16(0xD, 0x8939);
    cpu.y_register = 0x0010;
    cpu.mem_write(0x8949, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);

    cpu.mem_write(0xF, 0x01);
    cpu.mem_write(0x10, 0x0021);
    cpu.x_register = 0x0010;
    cpu.mem_write_u16(0x31, 0x8121);
    cpu.mem_write(0x8121, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);

    cpu.mem_write(0x11, 0x11);
    cpu.mem_write(0x12, 0x0031);
    cpu.mem_write_u16(0x0031, 0x8131);
    cpu.y_register = 0x0010;
    cpu.mem_write(0x8141, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);

    cpu.mem_write(0x13, 0x09);
    cpu.mem_write(0x14, 0b0000_0001);
    cpu.accumulator = 0b0000_0000;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b0000_0001);
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.mem_write(0x15, 0x09);
    cpu.mem_write(0x16, 0b0000_0000);
    cpu.accumulator = 0b0000_0000;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.mem_write(0x17, 0x09);
    cpu.mem_write(0x18, 0b0110_0000);
    cpu.accumulator = 0b1100_0000;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0b1110_0000);
    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
}

#[test]
fn test_ora_immediate() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x09);
    cpu.mem_write(0x1, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);
}

#[test]
fn test_ora_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x05);
    cpu.mem_write(0x1, 0x0025);
    cpu.mem_write(0x0025, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);
}

#[test]
fn test_ora_zero_page_x() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x15);
    cpu.mem_write(0x1, 0x0035);
    cpu.x_register = 0x0010;
    cpu.mem_write(0x0045, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);
}

#[test]
fn test_ora_absolute() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x0D);
    cpu.mem_write_u16(0x1, 0x8D2D);
    cpu.mem_write(0x8D2D, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);
}

#[test]
fn test_ora_absolute_x() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x1D);
    cpu.mem_write_u16(0x1, 0x8D3D);
    cpu.x_register = 0x0010;
    cpu.mem_write(0x8D4D, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);
}

#[test]
fn test_ora_absolute_y() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x19);
    cpu.mem_write_u16(0x1, 0x8939);
    cpu.y_register = 0x0010;
    cpu.mem_write(0x8949, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);
}

#[test]
fn test_ora_indirect_x() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x01);
    cpu.mem_write(0x1, 0x0021);
    cpu.x_register = 0x0010;
    cpu.mem_write_u16(0x31, 0x8121);
    cpu.mem_write(0x8121, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);
}

#[test]
fn test_ora_indirect_y() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x11);
    cpu.mem_write(0x1, 0x0031);
    cpu.mem_write_u16(0x0031, 0x8131);
    cpu.y_register = 0x0010;
    cpu.mem_write(0x8141, 0b0110_0110);

    cpu.accumulator = 0b1100_0011;
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0b1110_0111);
}

#[test]
fn test_ora_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x09);
    cpu.mem_write(0x1, 0b011_1111);
    cpu.accumulator = 0b0000_0000;

    cpu.step();
    cpu.step();

    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ora_flags_only_zero_when_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x09);
    cpu.mem_write(0x1, 0b0000_0000);
    cpu.accumulator = 0b0000_0000;

    cpu.step();
    cpu.step();

    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_ora_flags_only_negative_when_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0x09);
    cpu.mem_write(0x1, 0b0110_0000);
    cpu.accumulator = 0b1100_0000;

    cpu.step();
    cpu.step();

    assert!(cpu.get_negative_flag());
    assert!(!cpu.get_zero_flag());
}
