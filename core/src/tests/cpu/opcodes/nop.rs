use crate::tests::cpu::Cpu;

#[test]
fn test_nop() {
    let mut cpu = Cpu::test_instance();
    cpu.processor_status |= 0b0000_0001;
    cpu.accumulator = 0x66;

    cpu.mem_write(0x0, 0xEA);
    cpu.step();

    assert_eq!(cpu.accumulator, 0x66);
    assert_eq!(cpu.processor_status, 0b0000_0101);
}
