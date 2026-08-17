use crate::tests::cpu::Cpu;

#[test]
fn bcc_fail() {
    let mut cpu = Cpu::test_instance();
    cpu.processor_status |= 0b0000_0001;
    cpu.mem_write(0x0, 0xEA);
    cpu.mem_write(0x1, 0xEA);
    cpu.mem_write(0x2, 0xEA);
    cpu.mem_write(0x3, 0x90);
    cpu.mem_write(0x4, 0x12);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    cpu.step();
    cpu.step();

    assert_eq!(cpu.program_counter, 0x5);
}

#[test]
fn bcc_success() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xEA);
    cpu.mem_write(0x1, 0xEA);
    cpu.mem_write(0x2, 0xEA);
    cpu.mem_write(0x3, 0x90);
    cpu.mem_write(0x4, 0x12);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.program_counter, 0x17);

    cpu.mem_write(0x17, 0xA9);
    cpu.mem_write(0x18, 0x66);

    cpu.step();
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
}

#[test]
fn bcc_success_neg() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xEA);
    cpu.mem_write(0x1, 0xEA);
    cpu.mem_write(0x2, 0xEA);
    cpu.mem_write(0x3, 0x90);
    cpu.mem_write(0x4, 0xFE);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.program_counter, 0x3);
}
