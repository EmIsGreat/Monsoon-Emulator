use crate::tests::cpu::Cpu;

#[test]
fn test_txa_complete() {
    let mut cpu = Cpu::test_instance();
    cpu.x_register = 0x66;

    cpu.mem_write(0x0, 0x8A);

    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0x66);
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.x_register = 0x0;

    cpu.mem_write(0x1, 0x8A);

    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0x0);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());

    cpu.x_register = 0x80;

    cpu.mem_write(0x2, 0x8A);

    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0x80);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}

#[test]
fn test_txa_implied() {
    let mut cpu = Cpu::test_instance();
    cpu.x_register = 0x66;

    cpu.mem_write(0x0, 0x8A);

    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn test_txa_flags_none_when_none() {
    let mut cpu = Cpu::test_instance();
    cpu.x_register = 0x66;

    cpu.mem_write(0x0, 0x8A);

    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0x66);
    assert!(!cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_txa_flags_only_zero_when_zero() {
    let mut cpu = Cpu::test_instance();
    cpu.accumulator = 0x66;
    cpu.x_register = 0x0;

    cpu.mem_write(0x0, 0x8A);

    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0x0);
    assert!(cpu.get_zero_flag());
    assert!(!cpu.get_negative_flag());
}

#[test]
fn test_txa_flags_only_negative_when_negative() {
    let mut cpu = Cpu::test_instance();
    cpu.x_register = 0x80;

    cpu.mem_write(0x0, 0x8A);

    cpu.step();
    cpu.step();
    assert_eq!(cpu.accumulator, 0x80);
    assert!(!cpu.get_zero_flag());
    assert!(cpu.get_negative_flag());
}
