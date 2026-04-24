use std::collections::VecDeque;
use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::emulation::board::CpuBus;
use crate::emulation::nes::ExecutionFinished;
use crate::emulation::opcode;
use crate::emulation::opcode::{get_opcode, OpCode, OPCODES_MAP, OPCODES_TABLE};
use crate::emulation::savestate::CpuState;
use crate::util;

pub const INTERNAL_RAM_SIZE: u16 = 0x800;
pub const STACK_START_ADDRESS: u16 = 0x0100;
pub const NEGATIVE_BIT: u8 = 0x80;
pub const CARRY_BIT: u8 = 0x1;
pub const ZERO_BIT: u8 = 0x2;
pub const OVERFLOW_BIT: u8 = 0x40;
pub const IRQ_BIT: u8 = 0x4;
pub const UNUSED_BIT: u8 = 0x20;
pub const BREAK_BIT: u8 = 0x10;
pub const DECIMAL_BIT: u8 = 0x8;
pub const IRQ_VECTOR_ADDR: u16 = 0xFFFE;
pub const NMI_HANDLER_ADDR: u16 = 0xFFFA;
pub const RESET_VECTOR_ADDR: u16 = 0xFFFC;
pub const UPPER_BYTE: u16 = 0xFF00;
pub const LOWER_BYTE: u16 = 0x00FF;
pub const DMA_ADDRESS: u16 = 0x4014;
pub const OAM_REG_ADDRESS: u16 = 0x2004;

pub struct Cpu {
    pub program_counter: u16,
    pub stack_pointer: u8,
    pub accumulator: u8,
    pub x_register: u8,
    pub y_register: u8,
    pub processor_status: u8,
    pub lo: u8,
    pub hi: u8,
    pub current_op: MicroOp,
    pub op_queue: VecDeque<MicroOp>,
    pub current_opcode: Option<OpCode>,
    pub data_bus: u8,
    pub ane_constant: u8,
    pub is_halted: bool,
    pub irq_pending: bool,
    pub nmi_pending: bool,
    pub nmi_detected: bool,
    pub irq_detected: bool,
    pub locked_irq_vec: u16,
    pub current_irq_vec: u16,
    pub is_in_irq: bool,
    pub prev_nmi: bool,
    pub cpu_read_cycle: bool,
    pub dma_read: bool,
    pub dma_triggered: bool,
    pub dma_page: u8,
    /// Last memory access for watchpoint debugging (address, was_read, value)
    pub last_memory_access: Option<(u16, bool, u8)>,
    pub cycle: u128,
}

impl Default for Cpu {
    fn default() -> Self {
        // Initialize both HashMap and fast lookup table
        OPCODES_MAP.get_or_init(opcode::init);
        OPCODES_TABLE.get_or_init(opcode::init_lookup_table);

        Self {
            program_counter: 0,
            processor_status: 0x4,
            accumulator: 0,
            x_register: 0,
            y_register: 0,
            stack_pointer: 0,
            lo: 0,
            hi: 0,
            current_op: MicroOp::FetchOpcode,
            op_queue: VecDeque::with_capacity(8),
            current_opcode: None,
            data_bus: 0,
            ane_constant: 0xEE,
            is_halted: false,
            irq_pending: false,
            nmi_pending: false,
            nmi_detected: false,
            irq_detected: false,
            locked_irq_vec: 0,
            current_irq_vec: IRQ_VECTOR_ADDR,
            is_in_irq: false,
            prev_nmi: false,
            cpu_read_cycle: true,
            dma_read: true,
            dma_triggered: false,
            dma_page: 0,
            last_memory_access: None,
            cycle: 0,
        }
    }
}

impl Cpu {
    pub fn new() -> Self {
        OPCODES_MAP.get_or_init(opcode::init);
        OPCODES_TABLE.get_or_init(opcode::init_lookup_table);
        Self::default()
    }

    #[inline(always)]
    pub fn mem_read(&mut self, addr: u16, bus: &mut impl CpuBus) -> u8 {
        self.cpu_read_cycle = true;
        let res = bus.read(addr);

        self.last_memory_access = Some((addr, true, res));

        res
    }

    #[inline(always)]
    pub fn mem_write(&mut self, addr: u16, data: u8, bus: &mut impl CpuBus) {
        self.cpu_read_cycle = false;
        self.last_memory_access = Some((addr, false, data));

        if addr == DMA_ADDRESS {
            self.dma_triggered = true;
            self.dma_page = data;
        }

        bus.write(addr, data, self.cycle);
    }

    #[inline]
    pub fn mem_read_u16(&mut self, addr: u16, bus: &mut impl CpuBus) -> u16 {
        let least_significant_bits = self.mem_read(addr, bus) as u16;
        let highest_significant_bits = self.mem_read(addr + 1, bus) as u16;

        (highest_significant_bits << 8) | (least_significant_bits)
    }

    #[inline]
    pub fn mem_write_u16(&mut self, addr: u16, data: u16, bus: &mut impl CpuBus) {
        let least_significant_bits = (data & 0x00FF) as u8;
        let highest_significant_bits = (data >> 8) as u8;
        self.mem_write(addr, least_significant_bits, bus);
        self.mem_write(addr + 1, highest_significant_bits, bus)
    }

    #[inline]
    pub fn stack_pop(&mut self, bus: &mut impl CpuBus) -> u8 {
        let val = self.mem_read(STACK_START_ADDRESS + self.stack_pointer as u16, bus);
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        val
    }

    #[inline]
    pub fn stack_peek(&mut self, bus: &mut impl CpuBus) -> u8 {
        self.mem_read(STACK_START_ADDRESS + self.stack_pointer as u16, bus)
    }

    #[inline]
    pub fn stack_push(&mut self, data: Option<u8>, bus: &mut impl CpuBus) {
        if let Some(data) = data {
            let addr = STACK_START_ADDRESS + self.stack_pointer as u16;
            self.mem_write(addr, data, bus);
        }

        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    #[inline]
    pub fn stack_pop_u16(&mut self, bus: &mut impl CpuBus) -> u16 {
        let lo = self.stack_pop(bus) as u16;
        let hi = self.stack_pop(bus) as u16;
        (hi << 8) | lo
    }

    #[inline]
    pub fn stack_push_u16(&mut self, data: u16, bus: &mut impl CpuBus) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xFF) as u8;
        self.stack_push(Option::from(hi), bus);
        self.stack_push(Option::from(lo), bus);
    }

    #[inline]
    fn set_zero_flag(&mut self) { self.processor_status |= ZERO_BIT; }

    #[inline]
    fn clear_zero_flag(&mut self) { self.processor_status &= !ZERO_BIT; }

    #[inline]
    fn set_negative_flag(&mut self) { self.processor_status |= NEGATIVE_BIT }

    #[inline]
    fn clear_negative_flag(&mut self) { self.processor_status &= !NEGATIVE_BIT }

    #[inline]
    fn update_zero_flag(&mut self, result: u8) {
        if result == 0 {
            self.set_zero_flag()
        } else {
            self.clear_zero_flag()
        }
    }

    #[inline]
    fn update_negative_flag(&mut self, result: u8) {
        if result & NEGATIVE_BIT != 0 {
            self.set_negative_flag();
        } else {
            self.clear_negative_flag();
        }
    }

    #[inline]
    fn update_negative_and_zero_flags(&mut self, result: u8) {
        self.update_negative_flag(result);
        self.update_zero_flag(result);
    }

    #[inline]
    fn set_carry_flag(&mut self) { self.processor_status |= CARRY_BIT; }

    #[inline]
    fn clear_carry_flag(&mut self) { self.processor_status &= !CARRY_BIT; }

    #[inline]
    fn set_overflow_flag(&mut self) { self.processor_status |= OVERFLOW_BIT; }

    #[inline]
    fn clear_overflow_flag(&mut self) { self.processor_status &= !OVERFLOW_BIT; }

    #[inline]
    fn set_interrupt_disable(&mut self) { self.processor_status |= IRQ_BIT; }

    #[inline]
    fn clear_interrupt_disable(&mut self) { self.processor_status &= !IRQ_BIT; }

    #[inline]
    fn set_decimal_flag(&mut self) { self.processor_status |= DECIMAL_BIT; }

    #[inline]
    fn clear_decimal_flag(&mut self) { self.processor_status &= !DECIMAL_BIT; }

    #[inline]
    pub fn get_zero_flag(&self) -> bool { (self.processor_status & ZERO_BIT) != 0 }

    #[inline]
    pub fn get_negative_flag(&self) -> bool { (self.processor_status & NEGATIVE_BIT) != 0 }

    #[inline]
    pub fn get_carry_flag(&self) -> bool { (self.processor_status & CARRY_BIT) != 0 }

    #[inline]
    pub fn get_overflow_flag(&self) -> bool { (self.processor_status & OVERFLOW_BIT) != 0 }

    #[inline]
    pub fn get_decimal_flag(&self) -> bool { (self.processor_status & DECIMAL_BIT) != 0 }

    #[inline]
    pub fn get_interrupt_disable_flag(&self) -> bool { (self.processor_status & IRQ_BIT) != 0 }

    #[inline]
    pub fn get_break_flag(&self) -> bool { (self.processor_status & BREAK_BIT) != 0 }

    #[inline]
    pub fn get_unused_flag(&self) -> bool { (self.processor_status & UNUSED_BIT) != 0 }

    #[inline]
    fn shift_left(&mut self, data: u8) -> u8 {
        let res = data << 1;

        if data & NEGATIVE_BIT != 0 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        self.update_negative_and_zero_flags(res);
        res
    }

    #[inline]
    fn shift_right(&mut self, data: u8) -> u8 {
        let res = data >> 1;

        if data & CARRY_BIT != 0 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        self.update_negative_and_zero_flags(res);
        res
    }

    #[inline]
    fn rotate_left(&mut self, data: u8) -> u8 {
        let mut res = data << 1;

        res |= self.processor_status & CARRY_BIT;

        if data & NEGATIVE_BIT != 0 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        self.update_negative_and_zero_flags(res);
        res
    }

    #[inline]
    fn rotate_right(&mut self, data: u8) -> u8 {
        let mut res = data >> 1;

        if self.get_carry_flag() {
            res |= NEGATIVE_BIT;
        }

        if data & CARRY_BIT != 0 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        self.update_negative_and_zero_flags(res);
        res
    }

    #[inline]
    fn get_addr_latch(&self) -> u16 { ((self.hi as u16) << 8) | (self.lo as u16) }

    #[inline]
    fn get_instructions_for_op_type(&mut self) {
        self.op_queue.clear();
        let Some(op) = self.current_opcode else {
            return;
        };

        match op.op_type {
            OpType::AccumulatorOrImplied(callback) => {
                self.op_queue.push_back(MicroOp::DummyRead(callback))
            }
            OpType::ImmediateAddressing(target, callback) => {
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::PC,
                        Source::Constant(0),
                        target,
                        Source::None,
                        Source::None,
                        Target::None,
                        true,
                        callback,
                    ))
            }
            OpType::AbsoluteRead(target, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::FetchOperandHi(MicroOpCallback::None));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::AddressLatch,
                    target,
                    callback,
                ))
            }
            OpType::AbsoluteIndexRead(index, target, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::PC,
                        Source::Constant(0),
                        Target::HI,
                        Source::LO,
                        index,
                        Target::LO,
                        true,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::ReadPageCrossAware(
                    AddressSource::AddressLatch,
                    index,
                    target,
                    true,
                    callback,
                ))
            }
            OpType::ZeroPageRead(target, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::Read(AddressSource::LO, target, callback))
            }
            OpType::ZeroPageIndexRead(index, target, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::DummyReadAddOffsetWriteToTarget(
                        AddressSource::LO,
                        index,
                        Target::DataBus,
                        MicroOpCallback::None,
                    ));
                self.op_queue
                    .push_back(MicroOp::Read(AddressSource::Temp, target, callback))
            }
            OpType::IndexedIndirectRead(target, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::DummyReadAddOffsetWriteToTarget(
                        AddressSource::LO,
                        Source::X,
                        Target::DataBus,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::Temp,
                    Target::LO,
                    MicroOpCallback::None,
                ));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::Temp,
                        Source::Constant(1),
                        Target::HI,
                        Source::None,
                        Source::None,
                        Target::None,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::AddressLatch,
                    target,
                    callback,
                ))
            }
            OpType::IndirectIndexedRead(target, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::LO,
                    Target::DataBus,
                    MicroOpCallback::None,
                ));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::LO,
                        Source::Constant(1),
                        Target::HI,
                        Source::DataBus,
                        Source::Y,
                        Target::LO,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::ReadPageCrossAware(
                    AddressSource::AddressLatch,
                    Source::Y,
                    target,
                    true,
                    callback,
                ));
            }
            OpType::BRK(callback) => {
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::PC,
                        Source::Constant(0),
                        Target::None,
                        Source::None,
                        Source::None,
                        Target::None,
                        true,
                        MicroOpCallback::COPY(
                            AddressSource::Address(IRQ_VECTOR_ADDR),
                            Target::IrqVecCandidate,
                        ),
                    ));
                self.op_queue
                    .push_back(MicroOp::StackPush(Source::PCH, MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::StackPush(Source::PCL, MicroOpCallback::None));
                self.op_queue.push_back(MicroOp::StackPush(
                    Source::PBrk,
                    MicroOpCallback::LockIrqVec,
                ));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::IrqVec,
                    Target::PCL,
                    MicroOpCallback::SEI,
                ));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::IrqVec,
                        Source::Constant(1),
                        Target::PCH,
                        Source::None,
                        Source::None,
                        Target::None,
                        false,
                        callback,
                    ));
            }
            OpType::RTI(callback) => {
                self.op_queue
                    .push_back(MicroOp::DummyRead(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::None,
                        Source::None,
                        Target::None,
                        Source::SP,
                        Source::Constant(1),
                        Target::SP,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue
                    .push_back(MicroOp::StackPop(Target::P, MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::StackPop(Target::PCL, MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::StackPeek(Target::PCH, callback))
            }
            OpType::RTS(callback) => {
                self.op_queue
                    .push_back(MicroOp::DummyRead(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::None,
                        Source::None,
                        Target::None,
                        Source::SP,
                        Source::Constant(1),
                        Target::SP,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue
                    .push_back(MicroOp::StackPop(Target::PCL, MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::StackPeek(Target::PCH, MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::None,
                        Source::None,
                        Target::None,
                        Source::None,
                        Source::None,
                        Target::None,
                        true,
                        callback,
                    ))
            }
            OpType::PH(src, callback) => {
                self.op_queue
                    .push_back(MicroOp::DummyRead(MicroOpCallback::None));
                self.op_queue.push_back(MicroOp::StackPush(src, callback))
            }
            OpType::PL(target, callback) => {
                self.op_queue
                    .push_back(MicroOp::DummyRead(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::None,
                        Source::None,
                        Target::None,
                        Source::SP,
                        Source::Constant(1),
                        Target::SP,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue
                    .push_back(MicroOp::StackPeek(target, callback))
            }
            OpType::JSR(callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::DummyRead(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::StackPush(Source::PCH, MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::StackPush(Source::PCL, MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::PC,
                        Source::Constant(0),
                        Target::PCH,
                        Source::LO,
                        Source::Constant(0),
                        Target::PCL,
                        false,
                        callback,
                    ))
            }
            OpType::JmpAbsolute(callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::PC,
                        Source::Constant(0),
                        Target::PCH,
                        Source::LO,
                        Source::Constant(0),
                        Target::PCL,
                        false,
                        callback,
                    ))
            }
            OpType::AbsoluteRMW(target, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::FetchOperandHi(MicroOpCallback::None));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::AddressLatch,
                    target,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    Source::DataBus,
                    false,
                    callback,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    Source::DataBus,
                    false,
                    MicroOpCallback::None,
                ))
            }
            OpType::AbsoluteWrite(source, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::FetchOperandHi(MicroOpCallback::None));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    source,
                    true,
                    callback,
                ));
            }
            OpType::ZeroPageRMW(target, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::LO,
                    target,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::LoWrite,
                    Source::DataBus,
                    false,
                    callback,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::LoWrite,
                    Source::DataBus,
                    true,
                    MicroOpCallback::None,
                ))
            }
            OpType::ZeroPageWrite(source, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::Write(Target::LoWrite, source, true, callback));
            }
            OpType::ZeroPageIndexRMW(index, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::None,
                        Source::None,
                        Target::None,
                        Source::LO,
                        index,
                        Target::LO,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::LO,
                    Target::DataBus,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::LoWrite,
                    Source::DataBus,
                    false,
                    callback,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::LoWrite,
                    Source::DataBus,
                    true,
                    MicroOpCallback::None,
                ));
            }
            OpType::ZeroPageIndexWrite(source, index, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::None,
                        Source::None,
                        Target::None,
                        Source::LO,
                        index,
                        Target::LO,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue
                    .push_back(MicroOp::Write(Target::LoWrite, source, true, callback));
            }
            OpType::AbsoluteIndexRMW(offset, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::PC,
                        Source::Constant(0),
                        Target::HI,
                        Source::LO,
                        offset,
                        Target::LO,
                        true,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::ReadPageCrossAware(
                    AddressSource::AddressLatch,
                    offset,
                    Target::DataBus,
                    false,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::AddressLatch,
                    Target::DataBus,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    Source::DataBus,
                    false,
                    callback,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    Source::DataBus,
                    true,
                    MicroOpCallback::None,
                ));
            }
            OpType::AbsoluteIndexWrite(source, offset, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::PC,
                        Source::Constant(0),
                        Target::HI,
                        Source::LO,
                        offset,
                        Target::LO,
                        true,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::ReadPageCrossAware(
                    AddressSource::AddressLatch,
                    offset,
                    Target::None,
                    false,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    source,
                    true,
                    callback,
                ));
            }
            OpType::IndexedIndirectWrite(source, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::LO,
                        Source::Constant(0),
                        Target::None,
                        Source::LO,
                        Source::X,
                        Target::DataBus,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::Temp,
                    Target::LO,
                    MicroOpCallback::None,
                ));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::Temp,
                        Source::Constant(1),
                        Target::HI,
                        Source::None,
                        Source::None,
                        Target::None,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    source,
                    true,
                    callback,
                ));
            }
            OpType::JmpIndirect(callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::FetchOperandHi(MicroOpCallback::None));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::AddressLatch,
                    Target::DataBus,
                    MicroOpCallback::None,
                ));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::AddressLatch,
                        Source::Constant(1),
                        Target::PCH,
                        Source::DataBus,
                        Source::Constant(0),
                        Target::PCL,
                        false,
                        callback,
                    ));
            }
            OpType::IndirectIndexedWrite(source, callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::LO,
                        Source::Constant(0),
                        Target::DataBus,
                        Source::None,
                        Source::None,
                        Target::None,
                        false,
                        callback,
                    ));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::LO,
                        Source::Constant(1),
                        Target::HI,
                        Source::DataBus,
                        Source::Y,
                        Target::LO,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::ReadPageCrossAware(
                    AddressSource::AddressLatch,
                    Source::Y,
                    Target::None,
                    false,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    source,
                    true,
                    callback,
                ))
            }
            OpType::Relative(callback) => {
                self.op_queue.push_back(MicroOp::FetchOperandLo(callback));
            }
            OpType::IndexedIndirectRMW(callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::LO,
                        Source::Constant(0),
                        Target::None,
                        Source::LO,
                        Source::X,
                        Target::DataBus,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::Temp,
                    Target::LO,
                    MicroOpCallback::None,
                ));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
                        AddressSource::Temp,
                        Source::Constant(1),
                        Target::HI,
                        Source::None,
                        Source::None,
                        Target::None,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::AddressLatch,
                    Target::DataBus,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    Source::DataBus,
                    false,
                    callback,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    Source::DataBus,
                    true,
                    MicroOpCallback::None,
                ))
            }
            OpType::IndirectIndexedRMW(callback) => {
                self.op_queue
                    .push_back(MicroOp::FetchOperandLo(MicroOpCallback::None));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::LO,
                    Target::DataBus,
                    MicroOpCallback::None,
                ));
                self.op_queue
                    .push_back(MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                        AddressSource::LO,
                        Source::Constant(1),
                        Target::HI,
                        Source::DataBus,
                        Source::Y,
                        Target::LO,
                        false,
                        MicroOpCallback::None,
                    ));
                self.op_queue.push_back(MicroOp::ReadPageCrossAware(
                    AddressSource::AddressLatch,
                    Source::Y,
                    Target::DataBus,
                    false,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Read(
                    AddressSource::AddressLatch,
                    Target::DataBus,
                    MicroOpCallback::None,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    Source::DataBus,
                    false,
                    callback,
                ));
                self.op_queue.push_back(MicroOp::Write(
                    Target::AddressLatch,
                    Source::DataBus,
                    true,
                    MicroOpCallback::None,
                ))
            }
        }
    }

    #[inline]
    fn get_instructions_for_irq(&mut self) -> VecDeque<MicroOp> {
        let mut instructions = VecDeque::with_capacity(7);

        instructions.push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
            AddressSource::PC,
            Source::Constant(0),
            Target::None,
            Source::None,
            Source::None,
            Target::None,
            false,
            MicroOpCallback::None,
        ));
        instructions.push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
            AddressSource::PC,
            Source::Constant(0),
            Target::None,
            Source::None,
            Source::None,
            Target::None,
            false,
            MicroOpCallback::None,
        ));
        instructions.push_back(MicroOp::StackPush(Source::PCH, MicroOpCallback::None));
        instructions.push_back(MicroOp::StackPush(Source::PCL, MicroOpCallback::None));
        instructions.push_back(MicroOp::StackPush(
            Source::PIrq,
            MicroOpCallback::LockIrqVec,
        ));
        instructions.push_back(MicroOp::Read(
            AddressSource::IrqVec,
            Target::PCL,
            MicroOpCallback::SEI,
        ));
        instructions.push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
            AddressSource::IrqVec,
            Source::Constant(1),
            Target::PCH,
            Source::None,
            Source::None,
            Target::None,
            false,
            MicroOpCallback::ExitIrq,
        ));

        instructions
    }

    #[inline]
    fn get_instructions_for_reset(&mut self) -> VecDeque<MicroOp> {
        let mut instructions = VecDeque::with_capacity(7);

        instructions.push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
            AddressSource::PC,
            Source::Constant(0),
            Target::None,
            Source::None,
            Source::None,
            Target::None,
            false,
            MicroOpCallback::None,
        ));
        instructions.push_back(MicroOp::ReadWithOffsetFromU16AndAddSomething(
            AddressSource::PC,
            Source::Constant(0),
            Target::None,
            Source::None,
            Source::None,
            Target::None,
            false,
            MicroOpCallback::None,
        ));
        instructions.push_back(MicroOp::StackPush(Source::None, MicroOpCallback::None));
        instructions.push_back(MicroOp::StackPush(Source::None, MicroOpCallback::None));
        instructions.push_back(MicroOp::StackPush(Source::None, MicroOpCallback::None));
        instructions.push_back(MicroOp::Read(
            AddressSource::Address(RESET_VECTOR_ADDR),
            Target::PCL,
            MicroOpCallback::SEI,
        ));
        instructions.push_back(MicroOp::Read(
            AddressSource::Address(RESET_VECTOR_ADDR + 1),
            Target::PCH,
            MicroOpCallback::ExitIrq,
        ));

        instructions
    }

    #[inline]
    pub fn trigger_nmi(&mut self) {
        let mut seq = self.get_instructions_for_irq();
        self.nmi_pending = false;

        self.is_in_irq = true;

        debug_assert!(!seq.is_empty());
        if let Some(next) = seq.pop_front() {
            self.current_op = next;
        }
        self.op_queue = seq;
    }

    #[inline]
    pub fn trigger_irq(&mut self) {
        let mut seq = self.get_instructions_for_irq();

        self.is_in_irq = true;

        debug_assert!(!seq.is_empty());
        if let Some(next) = seq.pop_front() {
            self.current_op = next;
        }
        self.op_queue = seq;
    }

    #[inline]
    pub fn reset(&mut self) {
        if !self.is_in_irq {
            let mut seq = self.get_instructions_for_reset();

            self.is_in_irq = true;

            if let Some(next) = seq.pop_front() {
                self.current_op = next;
            }
            self.op_queue = seq;
        }
    }

    pub fn get_memory_debug(
        &self,
        range: Option<RangeInclusive<u16>>,
        bus: &impl CpuBus,
    ) -> Vec<u8> {
        let range = range.unwrap_or(0u16..=0xFFFF);
        let mut vec = Vec::with_capacity(range.len());
        range.for_each(|addr| vec.push(bus.read_debug(addr)));
        vec
    }

    #[inline(always)]
    pub fn step(
        &mut self,
        bus: &mut impl CpuBus,
    ) -> Result<ExecutionFinished, String> {
        self.cycle += 1;

        if self.is_halted {
            return Ok(ExecutionFinished {
                hlt_reached: true,
                ..Default::default()
            });
        }

        self.dma_read = !self.dma_read;

        let op = self.current_op;

        if !matches!(op, MicroOp::BranchIncrement(..)) && !self.is_in_irq && !self.dma_triggered {
            if self.nmi_detected {
                self.nmi_pending = true;
                self.nmi_detected = false;
            }

            self.irq_pending = self.irq_detected;
        }

        self.execute_micro_op(op, bus);

        if self.dma_triggered && self.cpu_read_cycle {
            self.trigger_oam_dma();
        }

        // NMI Things
        {
            bus.get_ppu_open_bus().tick(12);
            let curr_nmi = bus.poll_nmi();

            if curr_nmi && !self.prev_nmi {
                self.current_irq_vec = NMI_HANDLER_ADDR;
                self.nmi_detected = true;
            }

            self.prev_nmi = curr_nmi;
        }

        if bus.poll_irq() {
            self.current_irq_vec = IRQ_VECTOR_ADDR;
            self.irq_detected = true;
        } else {
            self.irq_detected = false;
        }

        if let Some(next_op) = self.op_queue.pop_front() {
            self.current_op = next_op;
        } else {
            if self.nmi_pending {
                self.trigger_nmi();
                self.nmi_pending = false;
                self.irq_pending = false;
                return Ok(ExecutionFinished {
                    cycle_completed: true,
                    ..Default::default()
                });
            } else if self.irq_pending && !self.get_interrupt_disable_flag() {
                self.trigger_irq();
                bus.set_irq(false);
                self.nmi_pending = false;
                self.irq_pending = false;
                return Ok(ExecutionFinished {
                    cycle_completed: true,
                    ..Default::default()
                });
            }

            self.current_op = MicroOp::FetchOpcode;
        }

        Ok(ExecutionFinished {
            cycle_completed: true,
            ..Default::default()
        })
    }

    #[inline(always)]
    fn execute_micro_op(&mut self, micro_op: MicroOp, bus: &mut impl CpuBus) {
        match micro_op {
            MicroOp::FetchOpcode => {
                let opcode = self.mem_read(self.program_counter, bus);
                self.program_counter = self.program_counter.wrapping_add(1);

                // Fast O(1) array lookup instead of HashMap
                self.current_opcode = get_opcode(opcode);

                self.get_instructions_for_op_type();
            }
            MicroOp::FetchOperandLo(callback) => {
                self.lo = self.mem_read(self.program_counter, bus);
                self.program_counter = self.program_counter.wrapping_add(1);

                self.run_op(callback, bus);
            }
            MicroOp::FetchOperandHi(callback) => {
                self.hi = self.mem_read(self.program_counter, bus);
                self.program_counter = self.program_counter.wrapping_add(1);

                self.run_op(callback, bus);
            }
            MicroOp::Read(source, target, callback) => {
                if let Some(address) = self.get_u16_address(&source) {
                    let val = self.mem_read(address, bus);
                    self.write_to_target(&target, val, bus);
                }

                self.run_op(callback, bus);
            }
            // pre_callback denotes whether to call the callback before or after the write. In the
            // case of a dummy write we write with false so that the value from before the callback
            // gets written, and in the case of a single write we need to execute the callback
            // beforehand so that we write the updated value
            MicroOp::Write(target, src, pre_callback, callback) => {
                if pre_callback {
                    self.run_op(callback, bus);
                }

                let val = self.get_src_value(&src);

                if let Some(val) = val {
                    self.write_to_target(&target, val, bus);
                }

                if !pre_callback {
                    self.run_op(callback, bus);
                }
            }
            MicroOp::StackPush(source, callback) => {
                let src_value = self.get_src_value(&source);

                self.stack_push(src_value, bus);

                self.run_op(callback, bus);
            }
            MicroOp::StackPop(target, callback) => {
                let val = self.stack_pop(bus);
                self.write_to_target(&target, val, bus);

                self.run_op(callback, bus);
            }
            MicroOp::StackPeek(target, callback) => {
                let val = self.stack_peek(bus);
                self.write_to_target(&target, val, bus);

                self.run_op(callback, bus);
            }
            MicroOp::ReadPageCrossAware(source, offset, target, schedule_read, callback) => {
                let mut page_cross = false;

                if let Some(address) = self.get_u16_address(&source) {
                    let val = self.mem_read(address, bus);
                    self.write_to_target(&target, val, bus);
                    let offset = self.get_src_value(&offset);

                    if let Some(offset) = offset
                        && self.lo.overflowing_sub(offset).1
                    {
                        page_cross = true;
                    }
                };

                if page_cross {
                    if schedule_read {
                        self.op_queue
                            .push_back(MicroOp::Read(source, target, callback));
                    }

                    self.hi = self.hi.wrapping_add(1);
                } else {
                    self.run_op(callback, bus);
                }
            }
            MicroOp::DummyReadAddOffsetWriteToTarget(source, offset, target, callback) => {
                if let Some(address) = self.get_u16_address(&source) {
                    self.mem_read(address, bus);
                    let src_value = self.get_src_value(&offset);

                    if let Some(src_value) = src_value {
                        self.write_to_target(
                            &target,
                            address.wrapping_add(src_value as u16) as u8,
                            bus,
                        );
                    }
                }

                self.run_op(callback, bus);
            }
            MicroOp::DummyRead(callback) => {
                self.mem_read(self.program_counter, bus);
                self.run_op(callback, bus);
            }
            MicroOp::ReadWithOffsetFromZPAndAddSomethingU8(
                address_source,
                offset,
                target,
                add_to_src,
                to_add,
                to_save,
                inc_pc,
                callback,
            ) => {
                if let Some(address) = self.get_u16_address(&address_source) {
                    let src_value = self.get_src_value(&offset);

                    if let Some(src_value) = src_value {
                        let offset_address = util::add_to_low_byte(address as u8 as u16, src_value);
                        let value = self.mem_read(offset_address, bus);
                        self.write_to_target(&target, value, bus);
                    }
                }

                let add_to = self.get_src_value(&add_to_src);
                let to_add = self.get_src_value(&to_add);

                if let Some(add_to) = add_to
                    && let Some(to_add) = to_add
                {
                    let value = add_to.wrapping_add(to_add);
                    self.write_to_target(&to_save, value, bus);
                }

                if inc_pc {
                    self.program_counter = self.program_counter.wrapping_add(1);
                }

                self.run_op(callback, bus);
            }
            MicroOp::ReadWithOffsetFromU16AndAddSomething(
                address_source,
                offset,
                target,
                add_to_src,
                to_add,
                to_save,
                inc_pc,
                callback,
            ) => {
                if let Some(address) = self.get_u16_address(&address_source) {
                    let offset = self.get_src_value(&offset);

                    if let Some(offset) = offset {
                        let offset_address = util::add_to_low_byte(address, offset);
                        let value = self.mem_read(offset_address, bus);
                        self.write_to_target(&target, value, bus);
                    }
                }

                let add_to = self.get_src_value(&add_to_src);
                let to_add = self.get_src_value(&to_add);

                if let Some(add_to) = add_to
                    && let Some(to_add) = to_add
                {
                    let value = add_to.wrapping_add(to_add);
                    self.write_to_target(&to_save, value, bus);
                }

                if inc_pc {
                    self.program_counter = self.program_counter.wrapping_add(1);
                }

                self.run_op(callback, bus);
            }
            MicroOp::BranchIncrement(to_add) => {
                let add_to = self.program_counter;
                let to_add = self.get_src_value(&to_add);

                if let Some(to_add) = to_add {
                    let value = add_to.wrapping_add(to_add as i8 as i16 as u16);
                    self.write_to_target(&Target::PCL, value as u8, bus);

                    if util::crosses_page_boundary_i8(add_to, to_add as i8) {
                        self.op_queue.push_back(MicroOp::FixHiBranch(value));
                    }
                }
            }
            MicroOp::FixHiBranch(value) => {
                self.program_counter = value;
            }
        }
    }

    #[inline]
    fn run_op(&mut self, op: MicroOpCallback, bus: &mut impl CpuBus) {
        match op {
            MicroOpCallback::None => {}
            MicroOpCallback::ADC => adc(self),
            MicroOpCallback::ASL => asl(self),
            MicroOpCallback::LSR => lsr(self),
            MicroOpCallback::ROL => rol(self),
            MicroOpCallback::ROR => ror(self),
            MicroOpCallback::CLC => clc(self),
            MicroOpCallback::CLD => cld(self),
            MicroOpCallback::CLI => cli(self),
            MicroOpCallback::CLV => clv(self),
            MicroOpCallback::DEX => dex(self),
            MicroOpCallback::DEY => dey(self),
            MicroOpCallback::INX => inx(self),
            MicroOpCallback::INY => iny(self),
            MicroOpCallback::SEI => sei(self),
            MicroOpCallback::SED => sed(self),
            MicroOpCallback::SEC => sec(self),
            MicroOpCallback::TAX => tax(self),
            MicroOpCallback::TAY => tay(self),
            MicroOpCallback::TSX => tsx(self),
            MicroOpCallback::TXA => txa(self),
            MicroOpCallback::TXS => txs(self),
            MicroOpCallback::TYA => tya(self),
            MicroOpCallback::AND => and(self),
            MicroOpCallback::CMP => cmp(self),
            MicroOpCallback::CPX => cpx(self),
            MicroOpCallback::CPY => cpy(self),
            MicroOpCallback::EOR => eor(self),
            MicroOpCallback::ORA => ora(self),
            MicroOpCallback::SBC => sbc(self),
            MicroOpCallback::BIT => bit(self),
            MicroOpCallback::DEC => dec(self),
            MicroOpCallback::INC => inc(self),
            MicroOpCallback::BRANCH(condition) => branch(self, condition),
            MicroOpCallback::ALR => alr(self),
            MicroOpCallback::ANC => anc(self),
            MicroOpCallback::ANC2 => anc(self),
            MicroOpCallback::ANE => ane(self),
            MicroOpCallback::ARR => arr(self),
            MicroOpCallback::DCP => dcp(self),
            MicroOpCallback::ISB => isb(self, bus),
            MicroOpCallback::LAX => lax(self),
            MicroOpCallback::LXA => lxa(self),
            MicroOpCallback::LAS => las(self),
            MicroOpCallback::RLA => rla(self),
            MicroOpCallback::RRA => rra(self),
            MicroOpCallback::SAX => sax(self),
            MicroOpCallback::SBX => sbx(self),
            MicroOpCallback::SHA => sha(self),
            MicroOpCallback::SHX => shx(self),
            MicroOpCallback::SHY => shy(self),
            MicroOpCallback::SLO => slo(self),
            MicroOpCallback::SRE => sre(self),
            MicroOpCallback::TAS => tas(self),
            MicroOpCallback::JAM => jam(self),
            MicroOpCallback::COPY(source, target) => copy(self, source, target),
            MicroOpCallback::LockIrqVec => self.locked_irq_vec = self.current_irq_vec,
            MicroOpCallback::SEIandLockIrqVec => {
                sei(self);
                self.locked_irq_vec = self.current_irq_vec
            }
            MicroOpCallback::ExitIrq => self.is_in_irq = false,
        }
    }

    #[inline]
    fn get_u16_address(&self, address_source: &AddressSource) -> Option<u16> {
        match address_source {
            AddressSource::AddressLatch => Some(self.get_addr_latch()),
            AddressSource::Address(u16) => Some(*u16),
            AddressSource::LO => Some(self.get_addr_latch() as u8 as u16),
            AddressSource::Temp => Some(self.data_bus as u16),
            AddressSource::None => None,
            AddressSource::HI => Some(self.hi as u16),
            AddressSource::PC => Some(self.program_counter),
            AddressSource::IrqVec => Some(self.locked_irq_vec),
        }
    }

    #[inline]
    pub fn get_src_value(&mut self, src: &Source) -> Option<u8> {
        match src {
            Source::A => Option::from(self.accumulator),
            Source::X => Option::from(self.x_register),
            Source::Y => Option::from(self.y_register),
            Source::SP => Option::from(self.stack_pointer),
            Source::PCL => Option::from((self.program_counter & LOWER_BYTE) as u8),
            Source::PCH => Option::from(((self.program_counter & UPPER_BYTE) >> 8) as u8),
            Source::LO => Option::from(self.lo),
            Source::HI => Option::from(self.hi),
            Source::DataBus => Option::from(self.data_bus),
            Source::Constant(val) => Option::from(*val),
            Source::None => None,
            Source::PBrk => Option::from(self.processor_status | (UNUSED_BIT | BREAK_BIT)),
            Source::PIrq => Option::from(self.processor_status | UNUSED_BIT),
        }
    }

    #[inline]
    fn write_to_target(&mut self, trg: &Target, val: u8, bus: &mut impl CpuBus) {
        match trg {
            Target::A => {
                self.accumulator = val;
                self.update_negative_and_zero_flags(self.accumulator)
            }
            Target::X => {
                self.x_register = val;
                self.update_negative_and_zero_flags(self.x_register)
            }
            Target::Y => {
                self.y_register = val;
                self.update_negative_and_zero_flags(self.y_register)
            }
            Target::SP => self.stack_pointer = val,
            Target::PCL => {
                self.program_counter = (self.program_counter & UPPER_BYTE) | (val as u16)
            }
            Target::PCH => {
                self.program_counter = (self.program_counter & LOWER_BYTE) | ((val as u16) << 8)
            }
            Target::LO => self.lo = val,
            Target::HI => self.hi = val,
            Target::DataBus => self.data_bus = val,
            Target::P => self.processor_status = val & (!UNUSED_BIT & !BREAK_BIT),
            Target::AddressLatch => {
                self.mem_write(self.get_addr_latch(), val, bus);
            }
            Target::LoWrite => self.mem_write(self.lo as u16, val, bus),
            Target::None => {}
            Target::OamWrite => self.mem_write(OAM_REG_ADDRESS, val, bus),
            Target::IrqVecCandidate => unreachable!(),
        }
    }

    #[inline]
    fn check_condition(&self, condition: Condition) -> bool {
        match condition {
            Condition::CarrySet => self.get_carry_flag(),
            Condition::CarryClear => !self.get_carry_flag(),
            Condition::ZeroSet => self.get_zero_flag(),
            Condition::ZeroClear => !self.get_zero_flag(),
            Condition::NegativeSet => self.get_negative_flag(),
            Condition::NegativeClear => !self.get_negative_flag(),
            Condition::OverflowSet => self.get_overflow_flag(),
            Condition::OverflowClear => !self.get_overflow_flag(),
        }
    }

    #[inline]
    pub fn trigger_oam_dma(&mut self) {
        self.dma_triggered = false;
        self.is_in_irq = true;
        let mut instr = VecDeque::new();

        instr.push_back(MicroOp::Read(
            AddressSource::None,
            Target::None,
            MicroOpCallback::None,
        ));

        if !self.dma_read {
            instr.push_back(MicroOp::Read(
                AddressSource::AddressLatch,
                Target::None,
                MicroOpCallback::None,
            ))
        }

        for oam_addr in 0x00u8..0xFFu8 {
            instr.push_back(MicroOp::Read(
                AddressSource::Address((self.dma_page as u16) << 8 | oam_addr as u16),
                Target::DataBus,
                MicroOpCallback::None,
            ));
            instr.push_back(MicroOp::Write(
                Target::OamWrite,
                Source::DataBus,
                false,
                MicroOpCallback::None,
            ))
        }

        instr.push_back(MicroOp::Read(
            AddressSource::Address(((self.dma_page as u16) << 8) | 0xFFu16),
            Target::DataBus,
            MicroOpCallback::None,
        ));
        instr.push_back(MicroOp::Write(
            Target::OamWrite,
            Source::DataBus,
            false,
            MicroOpCallback::ExitIrq,
        ));

        instr.append(&mut self.op_queue);
        self.op_queue = instr;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum MicroOp {
    FetchOpcode,
    FetchOperandLo(MicroOpCallback),
    FetchOperandHi(MicroOpCallback),
    Read(AddressSource, Target, MicroOpCallback),
    Write(Target, Source, bool, MicroOpCallback),
    StackPush(Source, MicroOpCallback),
    StackPop(Target, MicroOpCallback),
    /// When reading from AddressSource, assuming it was obtained by offsetting
    /// by Source, if an overflow occurred in its obtaining, increment hi to fix
    /// address latch
    ReadPageCrossAware(AddressSource, Source, Target, bool, MicroOpCallback),
    DummyReadAddOffsetWriteToTarget(AddressSource, Source, Target, MicroOpCallback),
    DummyRead(MicroOpCallback),
    ReadWithOffsetFromZPAndAddSomethingU8(
        AddressSource,
        Source,
        Target,
        Source,
        Source,
        Target,
        bool,
        MicroOpCallback,
    ),
    StackPeek(Target, MicroOpCallback),
    ReadWithOffsetFromU16AndAddSomething(
        AddressSource,
        Source,
        Target,
        Source,
        Source,
        Target,
        bool,
        MicroOpCallback,
    ),
    BranchIncrement(Source),
    FixHiBranch(u16),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum Target {
    A,
    X,
    Y,
    SP,
    PCL,
    PCH,
    LO,
    HI,
    DataBus,
    None,
    P,
    AddressLatch,
    LoWrite,
    IrqVecCandidate,
    OamWrite,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum Source {
    PBrk,
    A,
    X,
    Y,
    SP,
    PCL,
    PCH,
    LO,
    HI,
    DataBus,
    Constant(u8),
    None,
    PIrq,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum AddressSource {
    AddressLatch,
    Address(u16),
    LO,
    Temp,
    None,
    HI,
    PC,
    IrqVec,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum MicroOpCallback {
    None,
    ADC,
    ASL,
    LSR,
    ROL,
    ROR,
    CLC,
    CLD,
    CLI,
    CLV,
    DEX,
    DEY,
    INX,
    INY,
    SEI,
    SED,
    SEC,
    TAX,
    TAY,
    TSX,
    TXA,
    TXS,
    TYA,
    AND,
    CMP,
    CPX,
    CPY,
    EOR,
    ORA,
    SBC,
    BIT,
    DEC,
    INC,
    BRANCH(Condition),
    LAX,
    LXA,
    ALR,
    ANC,
    ANE,
    ARR,
    DCP,
    ISB,
    LAS,
    RLA,
    RRA,
    SAX,
    SBX,
    SHA,
    SHX,
    SHY,
    SLO,
    SRE,
    JAM,
    TAS,
    ANC2,
    COPY(AddressSource, Target),
    LockIrqVec,
    SEIandLockIrqVec,
    ExitIrq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Condition {
    CarrySet,
    CarryClear,
    ZeroSet,
    ZeroClear,
    NegativeSet,
    NegativeClear,
    OverflowSet,
    OverflowClear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum OpType {
    ImmediateAddressing(Target, MicroOpCallback),
    AbsoluteRead(Target, MicroOpCallback),
    AbsoluteIndexRead(Source, Target, MicroOpCallback),
    ZeroPageRead(Target, MicroOpCallback),
    ZeroPageIndexRead(Source, Target, MicroOpCallback),
    AccumulatorOrImplied(MicroOpCallback),
    IndexedIndirectRead(Target, MicroOpCallback),
    IndirectIndexedRead(Target, MicroOpCallback),
    BRK(MicroOpCallback),
    RTI(MicroOpCallback),
    RTS(MicroOpCallback),
    PH(Source, MicroOpCallback),
    PL(Target, MicroOpCallback),
    JSR(MicroOpCallback),
    JmpAbsolute(MicroOpCallback),
    AbsoluteRMW(Target, MicroOpCallback),
    AbsoluteWrite(Source, MicroOpCallback),
    ZeroPageRMW(Target, MicroOpCallback),
    ZeroPageWrite(Source, MicroOpCallback),
    ZeroPageIndexRMW(Source, MicroOpCallback),
    ZeroPageIndexWrite(Source, Source, MicroOpCallback),
    AbsoluteIndexRMW(Source, MicroOpCallback),
    AbsoluteIndexWrite(Source, Source, MicroOpCallback),
    IndexedIndirectWrite(Source, MicroOpCallback),
    JmpIndirect(MicroOpCallback),
    IndirectIndexedWrite(Source, MicroOpCallback),
    Relative(MicroOpCallback),
    IndexedIndirectRMW(MicroOpCallback),
    IndirectIndexedRMW(MicroOpCallback),
}

#[cfg(test)]
impl Cpu {
    pub fn test_instance() -> Self {
        let mut inst = Cpu::new();

        // Test instance doesn't get reset, therefore we need to manually fix the stack
        // pointer
        inst.stack_pointer = 0xFD;
        inst
    }
}

impl Cpu {
    pub fn from(state: &CpuState) -> Self {
        OPCODES_MAP.get_or_init(opcode::init);
        OPCODES_TABLE.get_or_init(opcode::init_lookup_table);

        Self {
            program_counter: state.program_counter,
            stack_pointer: state.stack_pointer,
            accumulator: state.accumulator,
            x_register: state.x_register,
            y_register: state.y_register,
            processor_status: state.processor_status,
            lo: state.lo,
            hi: state.hi,
            current_op: state.current_op,
            op_queue: state.op_queue.clone(),
            current_opcode: state.current_opcode.and_then(get_opcode),
            data_bus: state.data_bus,
            ane_constant: state.ane_constant,
            is_halted: state.is_halted,
            irq_pending: state.irq_pending,
            nmi_pending: state.nmi_pending,
            nmi_detected: state.nmi_detected,
            irq_detected: state.irq_detected,
            locked_irq_vec: state.locked_irq_vec,
            current_irq_vec: state.current_irq_vec,
            is_in_irq: state.is_in_irq,
            prev_nmi: state.prev_nmi,
            cpu_read_cycle: state.read_cycle,
            dma_read: state.dma_read,
            dma_triggered: state.dma_triggered,
            dma_page: state.dma_page,
            last_memory_access: None,
            cycle: state.cycle,
        }
    }
}

#[inline]
pub fn adc(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;
    let carry_in = cpu.processor_status & CARRY_BIT;

    let acc_check = cpu.accumulator;

    let sum = cpu.accumulator as u16 + target_value as u16 + carry_in as u16;
    let result = sum as u8;

    cpu.accumulator = result;

    // Carry Flag
    if sum > LOWER_BYTE {
        cpu.set_carry_flag();
    } else {
        cpu.clear_carry_flag();
    }

    // Overflow Flag
    if (!(acc_check ^ target_value) & (acc_check ^ result)) & NEGATIVE_BIT != 0 {
        cpu.set_overflow_flag();
    } else {
        cpu.clear_overflow_flag();
    }

    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn rol(cpu: &mut Cpu) {
    if !matches!(
        &cpu.current_opcode.unwrap().op_type,
        OpType::AccumulatorOrImplied(..)
    ) {
        let target_value = cpu.data_bus;
        let res = cpu.rotate_left(target_value);
        cpu.data_bus = res;
    } else {
        cpu.accumulator = cpu.rotate_left(cpu.accumulator);
        cpu.update_negative_and_zero_flags(cpu.accumulator);
    }
}

#[inline]
fn ror(cpu: &mut Cpu) {
    if !matches!(
        &cpu.current_opcode.unwrap().op_type,
        OpType::AccumulatorOrImplied(..)
    ) {
        let target_value = cpu.data_bus;
        let res = cpu.rotate_right(target_value);
        cpu.data_bus = res;
    } else {
        cpu.accumulator = cpu.rotate_right(cpu.accumulator);
        cpu.update_negative_and_zero_flags(cpu.accumulator);
    }
}

#[inline]
fn asl(cpu: &mut Cpu) {
    if !matches!(
        &cpu.current_opcode.unwrap().op_type,
        OpType::AccumulatorOrImplied(..)
    ) {
        let target_value = cpu.data_bus;
        let res = cpu.shift_left(target_value);
        cpu.data_bus = res;
    } else {
        cpu.accumulator = cpu.shift_left(cpu.accumulator);
        cpu.update_negative_and_zero_flags(cpu.accumulator);
    }
}

#[inline]
fn lsr(cpu: &mut Cpu) {
    if !matches!(
        &cpu.current_opcode.unwrap().op_type,
        OpType::AccumulatorOrImplied(..)
    ) {
        let target_value = cpu.data_bus;
        let res = cpu.shift_right(target_value);
        cpu.data_bus = res
    } else {
        cpu.accumulator = cpu.shift_right(cpu.accumulator);
        cpu.update_negative_and_zero_flags(cpu.accumulator);
    }
}

#[inline]
fn tax(cpu: &mut Cpu) {
    cpu.x_register = cpu.accumulator;
    cpu.update_negative_and_zero_flags(cpu.x_register);
}

#[inline]
fn tay(cpu: &mut Cpu) {
    cpu.y_register = cpu.accumulator;
    cpu.update_negative_and_zero_flags(cpu.y_register);
}

#[inline]
fn txa(cpu: &mut Cpu) {
    cpu.accumulator = cpu.x_register;
    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn tya(cpu: &mut Cpu) {
    cpu.accumulator = cpu.y_register;
    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn tsx(cpu: &mut Cpu) {
    cpu.x_register = cpu.stack_pointer;
    cpu.update_negative_and_zero_flags(cpu.x_register)
}

#[inline]
fn txs(cpu: &mut Cpu) { cpu.stack_pointer = cpu.x_register; }

#[inline]
fn clc(cpu: &mut Cpu) { cpu.clear_carry_flag(); }

#[inline]
fn cld(cpu: &mut Cpu) { cpu.clear_decimal_flag(); }

#[inline]
fn cli(cpu: &mut Cpu) { cpu.clear_interrupt_disable(); }

#[inline]
fn clv(cpu: &mut Cpu) { cpu.clear_overflow_flag(); }

#[inline]
fn sec(cpu: &mut Cpu) { cpu.set_carry_flag(); }

#[inline]
fn sed(cpu: &mut Cpu) { cpu.set_decimal_flag(); }

#[inline]
fn sei(cpu: &mut Cpu) { cpu.set_interrupt_disable(); }

#[inline]
fn dex(cpu: &mut Cpu) {
    let mod_value = cpu.x_register.wrapping_sub(1);
    cpu.x_register = mod_value;
    cpu.update_negative_and_zero_flags(cpu.x_register);
}

#[inline]
fn dey(cpu: &mut Cpu) {
    let mod_value = cpu.y_register.wrapping_sub(1);
    cpu.y_register = mod_value;
    cpu.update_negative_and_zero_flags(cpu.y_register);
}

#[inline]
fn inx(cpu: &mut Cpu) {
    let mod_value = cpu.x_register.wrapping_add(1);
    cpu.x_register = mod_value;
    cpu.update_negative_and_zero_flags(cpu.x_register);
}

#[inline]
fn iny(cpu: &mut Cpu) {
    let mod_value = cpu.y_register.wrapping_add(1);
    cpu.y_register = mod_value;
    cpu.update_negative_and_zero_flags(cpu.y_register);
}

#[inline]
fn cmp(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;

    if target_value == cpu.accumulator {
        cpu.set_zero_flag();
    } else {
        cpu.clear_zero_flag();
    }

    if cpu.accumulator >= target_value {
        cpu.set_carry_flag();
    } else {
        cpu.clear_carry_flag();
    }

    if cpu.accumulator.wrapping_sub(target_value) & NEGATIVE_BIT != 0 {
        cpu.set_negative_flag();
    } else {
        cpu.clear_negative_flag();
    }
}

#[inline]
fn cpx(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;

    if target_value == cpu.x_register {
        cpu.set_zero_flag();
    } else {
        cpu.clear_zero_flag();
    }

    if cpu.x_register >= target_value {
        cpu.set_carry_flag();
    } else {
        cpu.clear_carry_flag();
    }

    if cpu.x_register.wrapping_sub(target_value) & NEGATIVE_BIT != 0 {
        cpu.set_negative_flag();
    } else {
        cpu.clear_negative_flag();
    }
}

#[inline]
fn cpy(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;

    if target_value == cpu.y_register {
        cpu.set_zero_flag();
    } else {
        cpu.clear_zero_flag();
    }

    if cpu.y_register >= target_value {
        cpu.set_carry_flag();
    } else {
        cpu.clear_carry_flag();
    }

    if cpu.y_register.wrapping_sub(target_value) & NEGATIVE_BIT != 0 {
        cpu.set_negative_flag();
    } else {
        cpu.clear_negative_flag();
    }
}

#[inline]
fn and(cpu: &mut Cpu) {
    let target_val = cpu.data_bus;
    cpu.accumulator &= target_val;
    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn eor(cpu: &mut Cpu) {
    let target_val = cpu.data_bus;
    cpu.accumulator ^= target_val;
    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn ora(cpu: &mut Cpu) {
    let target_val = cpu.data_bus;
    cpu.accumulator |= target_val;
    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn sbc(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;
    let carry_in = cpu.processor_status & CARRY_BIT;

    let acc_check = cpu.accumulator;

    let value = target_value ^ LOWER_BYTE as u8;
    let sum = cpu.accumulator as u16 + value as u16 + carry_in as u16;
    let result = sum as u8;

    cpu.accumulator = result;

    if sum > LOWER_BYTE {
        cpu.set_carry_flag();
    } else {
        cpu.clear_carry_flag();
    }

    if ((value ^ result) & (acc_check ^ result) & NEGATIVE_BIT) != 0 {
        cpu.set_overflow_flag();
    } else {
        cpu.clear_overflow_flag();
    }

    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn bit(cpu: &mut Cpu) {
    let target_val = cpu.data_bus;
    let res = cpu.accumulator & target_val;
    cpu.update_zero_flag(res);

    if target_val & NEGATIVE_BIT != 0 {
        cpu.set_negative_flag()
    } else {
        cpu.clear_negative_flag()
    }

    if target_val & OVERFLOW_BIT != 0 {
        cpu.set_overflow_flag()
    } else {
        cpu.clear_overflow_flag()
    }
}

#[inline]
fn inc(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;
    let mod_value = target_value.wrapping_add(1);
    cpu.data_bus = mod_value;
    cpu.update_negative_and_zero_flags(cpu.data_bus);
}

#[inline]
fn dec(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;
    let mod_value = target_value.wrapping_sub(1);
    cpu.data_bus = mod_value;
    cpu.update_negative_and_zero_flags(cpu.data_bus);
}

#[inline]
fn branch(cpu: &mut Cpu, condition: Condition) {
    if cpu.check_condition(condition) {
        cpu.op_queue.push_back(MicroOp::BranchIncrement(Source::LO))
    }
}

#[inline]
fn isb(cpu: &mut Cpu, bus: &mut impl CpuBus) {
    // Inc
    let target_value = cpu.get_src_value(&Source::DataBus);

    if let Some(target_value) = target_value {
        let mod_value = target_value.wrapping_add(1);
        cpu.write_to_target(&Target::DataBus, mod_value, bus);
        cpu.update_negative_and_zero_flags(mod_value);

        // SBC
        let carry_in = cpu.processor_status & CARRY_BIT;

        let acc_check = cpu.accumulator;

        let value = mod_value ^ LOWER_BYTE as u8;
        let sum = cpu.accumulator as u16 + value as u16 + carry_in as u16;
        let result = sum as u8;

        cpu.accumulator = result;

        if sum > LOWER_BYTE {
            cpu.set_carry_flag();
        } else {
            cpu.clear_carry_flag();
        }

        if ((acc_check ^ result) & (value ^ result) & NEGATIVE_BIT) != 0 {
            cpu.set_overflow_flag();
        } else {
            cpu.clear_overflow_flag();
        }

        cpu.update_negative_and_zero_flags(cpu.accumulator);
    }
}

#[inline]
fn alr(cpu: &mut Cpu) {
    let target_val = cpu.data_bus;
    cpu.accumulator &= target_val;
    cpu.update_negative_and_zero_flags(cpu.accumulator);

    cpu.accumulator = cpu.shift_right(cpu.accumulator);
    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn anc(cpu: &mut Cpu) {
    let target_val = cpu.data_bus;
    cpu.accumulator &= target_val;
    cpu.update_negative_and_zero_flags(cpu.accumulator);

    if cpu.accumulator & NEGATIVE_BIT != 0 {
        cpu.set_carry_flag();
    } else {
        cpu.clear_carry_flag();
    }
}

#[inline]
fn ane(cpu: &mut Cpu) {
    cpu.accumulator = (cpu.accumulator | cpu.ane_constant) & cpu.x_register & cpu.data_bus;
}

#[inline]
fn arr(cpu: &mut Cpu) {
    let target_val = cpu.data_bus;
    cpu.accumulator &= target_val;

    cpu.accumulator = (cpu.accumulator >> 1)
        | (match cpu.get_carry_flag() {
            true => 1,
            false => 0,
        } << 7);

    cpu.update_negative_and_zero_flags(cpu.accumulator);

    if ((cpu.accumulator >> 6) & 1) != 0 {
        cpu.set_carry_flag();
    } else {
        cpu.clear_carry_flag();
    }

    if (((cpu.accumulator >> 6) & 1) ^ ((cpu.accumulator >> 5) & 1)) != 0 {
        cpu.set_overflow_flag();
    } else {
        cpu.clear_overflow_flag()
    }
}

#[inline]
fn dcp(cpu: &mut Cpu) {
    dec(cpu);
    cmp(cpu)
}

#[inline]
fn las(cpu: &mut Cpu) {
    let res = cpu.data_bus & cpu.stack_pointer;
    cpu.accumulator = res;
    cpu.x_register = res;
    cpu.stack_pointer = res;
}

#[inline]
fn lax(cpu: &mut Cpu) {
    cpu.accumulator = cpu.data_bus;
    cpu.x_register = cpu.data_bus;
    cpu.update_negative_and_zero_flags(cpu.accumulator)
}

#[inline]
fn lxa(cpu: &mut Cpu) {
    cpu.accumulator = (cpu.accumulator | cpu.ane_constant) & cpu.data_bus;
    cpu.x_register = cpu.accumulator;
    cpu.update_negative_and_zero_flags(cpu.accumulator)
}

#[inline]
fn rla(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;
    let res = cpu.rotate_left(target_value);
    cpu.data_bus = res;
    cpu.accumulator &= res;
    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn rra(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;
    let res = cpu.rotate_right(target_value);
    cpu.data_bus = res;
    adc(cpu);
}

#[inline]
fn sax(cpu: &mut Cpu) {
    let res = cpu.accumulator & cpu.x_register;
    cpu.data_bus = res;
}

#[inline]
fn sbx(cpu: &mut Cpu) {
    let t = cpu.accumulator & cpu.x_register;
    let r = t.wrapping_sub(cpu.data_bus);
    cpu.x_register = r;

    if t >= cpu.data_bus {
        cpu.set_carry_flag();
    } else {
        cpu.clear_carry_flag();
    }

    if r == 0 {
        cpu.set_zero_flag();
    } else {
        cpu.clear_zero_flag();
    }

    if (r & NEGATIVE_BIT) != 0 {
        cpu.set_negative_flag();
    } else {
        cpu.clear_negative_flag();
    }
}

#[inline]
fn sha(cpu: &mut Cpu) { cpu.data_bus = cpu.accumulator & cpu.x_register & cpu.hi.wrapping_add(1); }

#[inline]
fn shx(cpu: &mut Cpu) {
    if !cpu.lo.overflowing_sub(cpu.y_register).1 {
        cpu.data_bus = cpu.x_register & cpu.hi.wrapping_add(1);
    } else {
        let res = cpu.x_register & cpu.hi;
        cpu.hi = res;
        cpu.data_bus = res;
    }
}

#[inline]
fn shy(cpu: &mut Cpu) {
    if !cpu.lo.overflowing_sub(cpu.x_register).1 {
        cpu.data_bus = cpu.y_register & cpu.hi.wrapping_add(1);
    } else {
        let res = cpu.y_register & cpu.hi;
        cpu.hi = res;
        cpu.data_bus = res;
    }
}

#[inline]
fn slo(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;
    let res = cpu.shift_left(target_value);
    cpu.data_bus = res;

    cpu.accumulator |= res;
    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn sre(cpu: &mut Cpu) {
    let target_value = cpu.data_bus;
    let res = cpu.shift_right(target_value);
    cpu.data_bus = res;

    cpu.accumulator ^= res;
    cpu.update_negative_and_zero_flags(cpu.accumulator);
}

#[inline]
fn tas(cpu: &mut Cpu) {
    cpu.stack_pointer = cpu.accumulator & cpu.x_register;
    cpu.data_bus = cpu.accumulator & cpu.x_register & cpu.hi.wrapping_add(1);
}

#[inline]
fn jam(cpu: &mut Cpu) { cpu.is_halted = true }

#[inline]
fn copy(cpu: &mut Cpu, source: AddressSource, target: Target) {
    let Some(address) = cpu.get_u16_address(&source) else {
        unreachable!()
    };

    if target == Target::IrqVecCandidate {
        cpu.current_irq_vec = address
    }
}
