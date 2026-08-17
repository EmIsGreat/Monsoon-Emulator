use crate::tests::cpu::Cpu;

#[test]
fn test_dex_complete() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xCA);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.x_register, 0xFF);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());

    cpu.mem_write(0x1, 0xCA);
    cpu.x_register = 0x2;
    cpu.step();
    cpu.step();

    assert_eq!(cpu.x_register, 0x1);
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.mem_write(0x2, 0xCA);
    cpu.x_register = 0x1;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.x_register, 0x00);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.mem_write(0x3, 0xCA);
    cpu.x_register = 0xFF;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.x_register, 0xFE);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_dex_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xCA);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.x_register, 0xFF);
}

#[test]
fn test_dex_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xCA);
    cpu.x_register = 0x2;
    cpu.step();
    cpu.step();

    assert_eq!(cpu.x_register, 0x1);
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_dex_flags_zero_when_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xCA);
    cpu.x_register = 0x1;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.x_register, 0x00);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_dex_flags_negative_when_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xCA);
    cpu.x_register = 0xFF;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.x_register, 0xFE);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}
