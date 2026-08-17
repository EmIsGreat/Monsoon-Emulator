use crate::tests::cpu::Cpu;

#[test]
fn test_iny_complete() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC8);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.y_register, 0x1);
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.mem_write(0x1, 0xC8);
    cpu.y_register = 0xFF;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.y_register, 0x0);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.mem_write(0x2, 0xC8);
    cpu.y_register = 0x7F;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.y_register, 0x80);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_iny_zero_page() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC8);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.y_register, 0x1);
}

#[test]
fn test_iny_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC8);
    cpu.step();
    cpu.step();

    assert_eq!(cpu.y_register, 0x1);
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_iny_flags_zero_when_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC8);
    cpu.y_register = 0xFF;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.y_register, 0x0);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_iny_flags_negative_when_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xC8);
    cpu.y_register = 0x7F;

    cpu.step();
    cpu.step();

    assert_eq!(cpu.y_register, 0x80);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}
