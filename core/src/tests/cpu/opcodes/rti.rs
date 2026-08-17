use crate::tests::cpu::Cpu;

#[test]
fn test_rti() {
    let mut cpu = Cpu::test_instance();
    cpu.processor_status |= 0b0000_0001;
    cpu.stack_push(Some(0x10));
    cpu.stack_push(Some(0x80));
    cpu.stack_push(Option::from(cpu.processor_status));

    cpu.mem_write(0x0, 0x40);

    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();

    assert_eq!(cpu.program_counter, 0x1080);
}
