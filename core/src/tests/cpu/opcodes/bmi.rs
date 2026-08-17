use crate::tests::cpu::Cpu;

#[test]
fn bmi_fail() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xEA);
    cpu.mem_write(0x1, 0xEA);
    cpu.mem_write(0x2, 0xEA);
    cpu.mem_write(0x3, 0x30);
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
fn bmi_success() {
    let mut cpu = Cpu::test_instance();
    cpu.processor_status |= 0b1000_0000;
    cpu.mem_write(0x0, 0xEA);
    cpu.mem_write(0x1, 0xEA);
    cpu.mem_write(0x2, 0xEA);
    cpu.mem_write(0x3, 0x30);
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
