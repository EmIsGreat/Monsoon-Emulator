use crate::tests::cpu::Cpu;

#[test]
fn test_dec_complete() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC6);
    cpu.mem_write(0x1, 0x00C6);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00C6), 0xFF);

    cpu.mem_write(0x2, 0xD6);
    cpu.mem_write(0x3, 0x00D6);
    cpu.x_register = 0x10;

    cpu.mem_write(0x00E6, 0x3);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0x2);

    cpu.mem_write(0x4, 0xCE);
    cpu.mem_write_u16(0x5, 0xCECE);

    cpu.mem_write(0xCECE, 0xA9);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0xCECE), 0xA8);

    cpu.mem_write(0x7, 0xDE);
    cpu.mem_write_u16(0x8, 0xDEDE);
    cpu.x_register = 0x10;

    cpu.mem_write(0xDEEE, 0x2);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0xDEEE), 0x1);

    cpu.mem_write(0xA, 0xC6);
    cpu.mem_write(0xB, 0x00C7);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00C7), 0xFF);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());

    cpu.mem_write(0xC, 0xC6);
    cpu.mem_write(0xD, 0x00C7);
    cpu.mem_write(0x00C7, 0x1);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00C7), 0x0);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.mem_write(0xE, 0xC6);
    cpu.mem_write(0xF, 0x00C8);
    cpu.mem_write(0x00C8, 0x81);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00C8), 0x80);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_dec_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC6);
    cpu.mem_write(0x1, 0x00E6);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0xFF);
}

#[test]
fn test_dec_zero_page_x() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xD6);
    cpu.mem_write(0x1, 0x00E6);
    cpu.x_register = 0x10;

    cpu.mem_write(0x00F6, 0x3);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00F6), 0x2);
}

#[test]
fn test_dec_absolute() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xCE);
    cpu.mem_write_u16(0x1, 0xEEEE);

    cpu.mem_write(0xEEEE, 0xA9);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0xEEEE), 0xA8);
}

#[test]
fn test_dec_absolute_x() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xDE);
    cpu.mem_write_u16(0x1, 0xFEEE);
    cpu.x_register = 0x10;

    cpu.mem_write(0xFEFE, 0x2);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0xFEFE), 0x1);
}

#[test]
fn test_dec_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC6);
    cpu.mem_write(0x1, 0x00E6);
    cpu.mem_write(0x00E6, 0x2);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0x1);
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_dec_flags_zero_when_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC6);
    cpu.mem_write(0x1, 0x00E6);
    cpu.mem_write(0x00E6, 0x1);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0x0);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_dec_flags_negative_when_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC6);
    cpu.mem_write(0x1, 0x00E6);
    cpu.mem_write(0x00E6, 0x0);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0xFF);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}
