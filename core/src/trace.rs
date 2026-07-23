use std::fmt::Write;

use crate::emulation::board::CpuBus;
use crate::emulation::cpu::{Cpu, OpType, Source, UNUSED_BIT};
use crate::emulation::opcode;
use crate::emulation::opcode::OpCode;
use crate::util::add_to_low_byte;

#[derive(Clone, Debug)]
pub struct TraceLog {
    pub log: String,
}

pub struct CpuTraceState {
    program_counter: u16,
    accumulator: u8,
    x_register: u8,
    y_register: u8,
    processor_status: u8,
    stack_pointer: u8,
    current_opcode: Option<OpCode>,
}

impl Default for TraceLog {
    fn default() -> Self { Self::new() }
}

impl TraceLog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            log: String::new(),
        }
    }

    pub fn trace(&mut self, cpu: &Cpu, bus: &impl CpuBus, total_cycles: u64) {
        let current_opcode = cpu.current_opcode;

        let cpu = CpuTraceState {
            program_counter: cpu.program_counter,
            accumulator: cpu.accumulator,
            x_register: cpu.x_register,
            y_register: cpu.y_register,
            processor_status: cpu.processor_status,
            stack_pointer: cpu.stack_pointer,
            current_opcode: Some(current_opcode),
        };

        let relevant_mem_start = cpu.program_counter.wrapping_sub(1);
        let relevant_mem_end = relevant_mem_start
            .wrapping_add(u16::from(opcode::get_bytes_for_opcode(current_opcode)));
        let relevant_mem: Vec<u8> = bus.get_range(relevant_mem_start..=relevant_mem_end);

        let mut mem_formatted = String::with_capacity(8);
        for (idx, byte) in relevant_mem.iter().enumerate() {
            if idx != 0 {
                mem_formatted.push(' ');
            }
            let _ = write!(&mut mem_formatted, "{byte:02X}");
        }

        let descriptor_string = get_opcode_descriptor(current_opcode, &cpu, bus);

        let _ = writeln!(
            &mut self.log,
            "{:04X}  {:<8} {:>4} {:<27} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{}",
            cpu.program_counter.wrapping_sub(1),
            mem_formatted,
            current_opcode.name,
            descriptor_string,
            cpu.accumulator,
            cpu.x_register,
            cpu.y_register,
            cpu.processor_status | UNUSED_BIT,
            cpu.stack_pointer,
            total_cycles / 12
        );
    }
}

#[must_use]
pub fn get_str_for_src(source: Source) -> String {
    match source {
        Source::X => String::from("X"),
        Source::Y => String::from("Y"),
        _ => String::new(),
    }
}

#[allow(clippy::too_many_lines)]
pub fn get_opcode_descriptor(opcode: OpCode, cpu: &CpuTraceState, bus: &impl CpuBus) -> String {
    match opcode.op_type {
        OpType::ImmediateAddressing(..) => {
            format!("#${:02X}", bus.read_debug(cpu.program_counter))
        }
        OpType::AccumulatorOrImplied(..) => {
            if cpu.current_opcode.is_none() {
                return String::new();
            }

            match opcode {
                OpCode {
                    name: "LSR" | "ASL" | "ROR" | "ROL",
                    ..
                } => String::from("A"),
                _ => String::new(),
            }
        }
        OpType::AbsoluteRead(..) | OpType::AbsoluteRMW(..) | OpType::AbsoluteWrite(..) => {
            let address = (u16::from(bus.read_debug(cpu.program_counter.wrapping_add(1))) << 8)
                | u16::from(bus.read_debug(cpu.program_counter));
            format!("${:04X} = {:02X}", address, bus.read_debug(address))
        }
        OpType::AbsoluteIndexRead(source, ..)
        | OpType::AbsoluteIndexRMW(source, ..)
        | OpType::AbsoluteIndexWrite(_, source, ..) => {
            let address = u16::from(bus.read_debug(cpu.program_counter.wrapping_add(1))) << 8
                | u16::from(bus.read_debug(cpu.program_counter));

            let reg_string = get_str_for_src(source);

            let val = match source {
                Source::X => cpu.x_register,
                Source::Y => cpu.y_register,
                _ => 0,
            };

            let effective_address = address.wrapping_add(u16::from(val));

            format!(
                "${:04X},{} @ {:04X} = {:02X}",
                address,
                reg_string,
                effective_address,
                bus.read_debug(effective_address)
            )
        }
        OpType::ZeroPageRead(..) | OpType::ZeroPageRMW(..) | OpType::ZeroPageWrite(..) => {
            let address = bus.read_debug(cpu.program_counter);
            format!(
                "${:02X} = {:02X}",
                address,
                bus.read_debug(u16::from(address))
            )
        }
        OpType::ZeroPageIndexRead(source, ..)
        | OpType::ZeroPageIndexRMW(source, ..)
        | OpType::ZeroPageIndexWrite(_, source, ..) => {
            let address = bus.read_debug(cpu.program_counter);

            let reg_string = get_str_for_src(source);

            let val = match source {
                Source::X => cpu.x_register,
                Source::Y => cpu.y_register,
                _ => 0,
            };

            let effective_address = address.wrapping_add(val);

            format!(
                "${:02X},{} @ {:02X} = {:02X}",
                address,
                reg_string,
                effective_address,
                bus.read_debug(u16::from(effective_address))
            )
        }
        OpType::IndexedIndirectRead(..)
        | OpType::IndexedIndirectRMW(_)
        | OpType::IndexedIndirectWrite(..) => {
            let address = bus.read_debug(cpu.program_counter);

            let effective_address = address.wrapping_add(cpu.x_register);
            let lookup_addr =
                (u16::from(bus.read_debug(u16::from(effective_address.wrapping_add(1)))) << 8)
                    | u16::from(bus.read_debug(u16::from(effective_address)));

            let val = bus.read_debug(lookup_addr);

            format!("(${address:02X},X) @ {effective_address:02X} = {lookup_addr:04X} = {val:02X}")
        }
        OpType::IndirectIndexedRead(..)
        | OpType::IndirectIndexedRMW(_)
        | OpType::IndirectIndexedWrite(..) => {
            let address = bus.read_debug(cpu.program_counter);

            let effective_addr = (u16::from(bus.read_debug(u16::from(address.wrapping_add(1))))
                << 8)
                | u16::from(bus.read_debug(u16::from(address)));

            let lookup_addr = effective_addr.wrapping_add(u16::from(cpu.y_register));

            let val = bus.read_debug(lookup_addr);

            format!("(${address:02X}),Y = {effective_addr:04X} @ {lookup_addr:04X} = {val:02X}")
        }
        OpType::BRK(_) | OpType::RTI(..) | OpType::RTS(..) | OpType::PH(..) | OpType::PL(..) => {
            String::new()
        }
        OpType::JSR(_) | OpType::JmpAbsolute(_) => {
            let address = (u16::from(bus.read_debug(cpu.program_counter.wrapping_add(1))) << 8)
                | u16::from(bus.read_debug(cpu.program_counter));
            format!("${address:04X}")
        }
        OpType::JmpIndirect(_) => {
            let address = (u16::from(bus.read_debug(cpu.program_counter.wrapping_add(1))) << 8)
                | u16::from(bus.read_debug(cpu.program_counter));

            let val = (u16::from(bus.read_debug(add_to_low_byte(address, 1))) << 8)
                | u16::from(bus.read_debug(address));
            format!("(${address:04X}) = {val:04X}")
        }
        OpType::Relative(..) => {
            let base_address = cpu.program_counter.wrapping_add(1);
            let offset = bus.read_debug(cpu.program_counter).cast_signed();
            let val = base_address.wrapping_add_signed(i16::from(offset));

            format!("${val:04X}")
        }
    }
}
