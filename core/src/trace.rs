use crate::emulation::board::{Board, CpuBus, CpuBusView};
use crate::emulation::cpu::{
    OpType, Source, UNUSED_BIT
    ,
};
use crate::emulation::opcode;
use crate::emulation::opcode::{get_opcode, OpCode};
use crate::emulation::savestate::SaveState;
use crate::util::add_to_low_byte;

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
    pub fn new() -> Self {
        Self {
            log: String::from(""),
        }
    }

    pub fn trace(&mut self, nes: SaveState) {
        let mut board = Board::from(&nes.board);
        let cpu = CpuTraceState {
            program_counter: board.cpu.program_counter,
            accumulator: board.cpu.accumulator,
            x_register: board.cpu.x_register,
            y_register: board.cpu.y_register,
            processor_status: board.cpu.processor_status,
            stack_pointer: board.cpu.stack_pointer,
            current_opcode: board.cpu.current_opcode,
        };
        let current_opcode = get_opcode(cpu.current_opcode.unwrap().opcode).unwrap();

        let relevant_mem_start = cpu.program_counter.wrapping_sub(1);
        let relevant_mem_end =
            relevant_mem_start.wrapping_add(opcode::get_bytes_for_opcode(current_opcode) as u16);
        let bus = CpuBusView::from(
            &mut board.mapper,
            &mut board.cpu_open_bus,
            &mut board.ppu_open_bus,
            &mut board.cpu_ram,
            &mut board.nametable_ram,
            &mut board.palette_ram,
            &mut board.ppu,
            &mut board.irq,
            &mut board.controller1,
            &mut board.controller2,
            &mut board.joystick_strobe_data,
        );
        let relevant_mem: Vec<u8> = bus.get_range(relevant_mem_start..=relevant_mem_end);
        let mem_formatted = relevant_mem
            .iter()
            .map(|n| format!("{:02X}", n))
            .collect::<Vec<_>>()
            .join(" ");

        let descriptor_string = get_opcode_descriptor(current_opcode, &cpu, &bus);

        self.log += format!(
            "{:04X}  {:<8} {:>4} {:<27} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{}\n",
            cpu.program_counter.wrapping_sub(1),
            mem_formatted,
            current_opcode.name,
            descriptor_string,
            cpu.accumulator,
            cpu.x_register,
            cpu.y_register,
            cpu.processor_status | UNUSED_BIT,
            cpu.stack_pointer,
            nes.total_cycles / 12
        )
        .as_str();
    }
}

pub fn get_str_for_src(source: Source) -> String {
    match source {
        Source::X => String::from("X"),
        Source::Y => String::from("Y"),
        Source::None => String::new(),
        _ => String::new(),
    }
}

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
            let address = ((bus.read_debug(cpu.program_counter.wrapping_add(1)) as u16) << 8)
                | bus.read_debug(cpu.program_counter) as u16;
            format!("${:04X} = {:02X}", address, bus.read_debug(address))
        }
        OpType::AbsoluteIndexRead(source, ..)
        | OpType::AbsoluteIndexRMW(source, ..)
        | OpType::AbsoluteIndexWrite(_, source, ..) => {
            let address = (bus.read_debug(cpu.program_counter.wrapping_add(1)) as u16) << 8
                | (bus.read_debug(cpu.program_counter) as u16);

            let reg_string = get_str_for_src(source);

            let val = match source {
                Source::X => cpu.x_register,
                Source::Y => cpu.y_register,
                _ => 0,
            };

            let effective_address = address.wrapping_add(val as u16);

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
            format!("${:02X} = {:02X}", address, bus.read_debug(address as u16))
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
                bus.read_debug(effective_address as u16)
            )
        }
        OpType::IndexedIndirectRead(..)
        | OpType::IndexedIndirectRMW(_)
        | OpType::IndexedIndirectWrite(..) => {
            let address = bus.read_debug(cpu.program_counter);

            let effective_address = address.wrapping_add(cpu.x_register);
            let lookup_addr = ((bus.read_debug(effective_address.wrapping_add(1) as u16) as u16)
                << 8)
                | bus.read_debug(effective_address as u16) as u16;

            let val = bus.read_debug(lookup_addr);

            format!(
                "(${:02X},X) @ {:02X} = {:04X} = {:02X}",
                address, effective_address, lookup_addr, val
            )
        }
        OpType::IndirectIndexedRead(..)
        | OpType::IndirectIndexedRMW(_)
        | OpType::IndirectIndexedWrite(..) => {
            let address = bus.read_debug(cpu.program_counter);

            let effective_addr = ((bus.read_debug(address.wrapping_add(1) as u16) as u16) << 8)
                | bus.read_debug(address as u16) as u16;

            let lookup_addr = effective_addr.wrapping_add(cpu.y_register as u16);

            let val = bus.read_debug(lookup_addr);

            format!(
                "(${:02X}),Y = {:04X} @ {:04X} = {:02X}",
                address, effective_addr, lookup_addr, val
            )
        }
        OpType::BRK(_) | OpType::RTI(..) | OpType::RTS(..) | OpType::PH(..) | OpType::PL(..) => {
            String::new()
        }
        OpType::JSR(_) | OpType::JmpAbsolute(_) => {
            let address = ((bus.read_debug(cpu.program_counter.wrapping_add(1)) as u16) << 8)
                | bus.read_debug(cpu.program_counter) as u16;
            format!("${:04X}", address)
        }
        OpType::JmpIndirect(_) => {
            let address = ((bus.read_debug(cpu.program_counter.wrapping_add(1)) as u16) << 8)
                | bus.read_debug(cpu.program_counter) as u16;

            let val = ((bus.read_debug(add_to_low_byte(address, 1)) as u16) << 8)
                | bus.read_debug(address) as u16;
            format!("(${:04X}) = {:04X}", address, val)
        }
        OpType::Relative(..) => {
            let base_address = cpu.program_counter.wrapping_add(1);
            let offset = bus.read_debug(cpu.program_counter) as i8;
            let val = base_address.wrapping_add(offset as i16 as u16);

            format!("${:04X}", val)
        }
    }
}
