use crate::tests::cpu::Cpu;

#[test]
fn test_inc_complete() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xE6);
    cpu.mem_write(0x1, 0x00E6);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0x1);

    cpu.mem_write(0x2, 0xF6);
    cpu.mem_write(0x3, 0x00E6);
    cpu.x_register = 0x10;

    cpu.mem_write(0x00F6, 0x3);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00F6), 0x4);

    cpu.mem_write(0x4, 0xEE);
    cpu.mem_write_u16(0x5, 0xEEEE);

    cpu.mem_write(0xEEEE, 0xA9);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0xEEEE), 0xAA);

    cpu.mem_write(0x7, 0xFE);
    cpu.mem_write_u16(0x8, 0xFEEE);
    cpu.x_register = 0x10;

    cpu.mem_write(0xFEFE, 0x2);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0xFEFE), 0x3);

    cpu.mem_write(0xA, 0xE6);
    cpu.mem_write(0xB, 0x00E7);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E7), 0x1);
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.mem_write(0xC, 0xE6);
    cpu.mem_write(0xD, 0x00E6);
    cpu.mem_write(0x00E6, 0xFF);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0x0);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.mem_write(0xE, 0xE6);
    cpu.mem_write(0xF, 0x00E6);
    cpu.mem_write(0x00E6, 0x7F);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0x80);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_inc_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xE6);
    cpu.mem_write(0x1, 0x00E6);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0x1);
}

#[test]
fn test_inc_zero_page_x() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xF6);
    cpu.mem_write(0x1, 0x00E6);
    cpu.x_register = 0x10;

    cpu.mem_write(0x00F6, 0x3);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00F6), 0x4);
}

#[test]
fn test_inc_absolute() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xEE);
    cpu.mem_write_u16(0x1, 0xEEEE);

    cpu.mem_write(0xEEEE, 0xA9);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0xEEEE), 0xAA);
}

#[test]
fn test_inc_absolute_x() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xFE);
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

    assert_eq!(cpu.mem_read(0xFEFE), 0x3);
}

#[test]
fn test_inc_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xE6);
    cpu.mem_write(0x1, 0x00E6);
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
fn test_inc_flags_zero_when_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xE6);
    cpu.mem_write(0x1, 0x00E6);
    cpu.mem_write(0x00E6, 0xFF);
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
fn test_inc_flags_negative_when_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xE6);
    cpu.mem_write(0x1, 0x00E6);
    cpu.mem_write(0x00E6, 0x7F);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.mem_read(0x00E6), 0x80);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}
