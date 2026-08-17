use crate::tests::cpu::Cpu;

#[test]
fn bcs_fail() {
    let mut cpu = Cpu::test_instance();
    cpu.mem_write(0x0, 0xEA);
    cpu.mem_write(0x1, 0xEA);
    cpu.mem_write(0x2, 0xEA);
    cpu.mem_write(0x3, 0xB0);
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
fn bcs_success() {
    let mut cpu = Cpu::test_instance();
    cpu.processor_status |= 0b0000_0001;
    cpu.mem_write(0x0, 0xEA);
    cpu.mem_write(0x1, 0xEA);
    cpu.mem_write(0x2, 0xEA);
    cpu.mem_write(0x3, 0xB0);
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
