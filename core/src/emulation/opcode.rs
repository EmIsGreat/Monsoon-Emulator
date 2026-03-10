use std::collections::HashMap;
use std::sync::OnceLock;

use crate::emulation::cpu::OpType::{
    AbsoluteIndexRMW, AbsoluteIndexRead, AbsoluteIndexWrite, AbsoluteRMW, AbsoluteRead,
    AbsoluteWrite, AccumulatorOrImplied, BRK, ImmediateAddressing, IndexedIndirectRMW,
    IndexedIndirectRead, IndexedIndirectWrite, IndirectIndexedRMW, IndirectIndexedRead,
    IndirectIndexedWrite, JSR, JmpAbsolute, JmpIndirect, PH, PL, RTI, RTS, Relative,
    ZeroPageIndexRMW, ZeroPageIndexRead, ZeroPageIndexWrite, ZeroPageRMW, ZeroPageRead,
    ZeroPageWrite,
};
use crate::emulation::cpu::{Condition, MicroOpCallback, OpType, Source, Target};

pub static OPCODES: OnceLock<Vec<OpCode>> = OnceLock::new();
pub static OPCODES_MAP: OnceLock<HashMap<u8, &'static OpCode>> = OnceLock::new();
/// Direct lookup table for opcodes - O(1) array access vs HashMap
pub static OPCODES_TABLE: OnceLock<[Option<OpCode>; 256]> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct OpCode {
    pub opcode: u8,
    pub name: &'static str,
    pub op_type: OpType,
}

#[inline]
pub fn init() -> HashMap<u8, &'static OpCode> {
    OPCODES
        .set(vec![
            // ADC
            OpCode::new(
                0x69,
                "ADC",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::ADC),
            ),
            OpCode::new(
                0x65,
                "ADC",
                ZeroPageRead(Target::DataBus, MicroOpCallback::ADC),
            ),
            OpCode::new(
                0x75,
                "ADC",
                ZeroPageIndexRead(Source::X, Target::DataBus, MicroOpCallback::ADC),
            ),
            OpCode::new(
                0x6D,
                "ADC",
                AbsoluteRead(Target::DataBus, MicroOpCallback::ADC),
            ),
            OpCode::new(
                0x7D,
                "ADC",
                AbsoluteIndexRead(Source::X, Target::DataBus, MicroOpCallback::ADC),
            ),
            OpCode::new(
                0x79,
                "ADC",
                AbsoluteIndexRead(Source::Y, Target::DataBus, MicroOpCallback::ADC),
            ),
            OpCode::new(
                0x61,
                "ADC",
                IndexedIndirectRead(Target::DataBus, MicroOpCallback::ADC),
            ),
            OpCode::new(
                0x71,
                "ADC",
                IndirectIndexedRead(Target::DataBus, MicroOpCallback::ADC),
            ),
            // AND
            OpCode::new(
                0x29,
                "AND",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::AND),
            ),
            OpCode::new(
                0x25,
                "AND",
                ZeroPageRead(Target::DataBus, MicroOpCallback::AND),
            ),
            OpCode::new(
                0x35,
                "AND",
                ZeroPageIndexRead(Source::X, Target::DataBus, MicroOpCallback::AND),
            ),
            OpCode::new(
                0x2D,
                "AND",
                AbsoluteRead(Target::DataBus, MicroOpCallback::AND),
            ),
            OpCode::new(
                0x3D,
                "AND",
                AbsoluteIndexRead(Source::X, Target::DataBus, MicroOpCallback::AND),
            ),
            OpCode::new(
                0x39,
                "AND",
                AbsoluteIndexRead(Source::Y, Target::DataBus, MicroOpCallback::AND),
            ),
            OpCode::new(
                0x21,
                "AND",
                IndexedIndirectRead(Target::DataBus, MicroOpCallback::AND),
            ),
            OpCode::new(
                0x31,
                "AND",
                IndirectIndexedRead(Target::DataBus, MicroOpCallback::AND),
            ),
            // ASL
            OpCode::new(0x0A, "ASL", AccumulatorOrImplied(MicroOpCallback::ASL)),
            OpCode::new(0x06, "ASL", ZeroPageRMW(Target::DataBus, MicroOpCallback::ASL)),
            OpCode::new(
                0x16,
                "ASL",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::ASL),
            ),
            OpCode::new(0x0E, "ASL", AbsoluteRMW(Target::DataBus, MicroOpCallback::ASL)),
            OpCode::new(
                0x1E,
                "ASL",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::ASL),
            ),
            // BCC
            OpCode::new(
                0x90,
                "BCC",
                Relative(MicroOpCallback::BRANCH(Condition::CarryClear)),
            ),
            // BCS
            OpCode::new(
                0xB0,
                "BCS",
                Relative(MicroOpCallback::BRANCH(Condition::CarrySet)),
            ),
            // BEQ
            OpCode::new(
                0xF0,
                "BEQ",
                Relative(MicroOpCallback::BRANCH(Condition::ZeroSet)),
            ),
            // BIT
            OpCode::new(
                0x24,
                "BIT",
                ZeroPageRead(Target::DataBus, MicroOpCallback::BIT),
            ),
            OpCode::new(
                0x2C,
                "BIT",
                AbsoluteRead(Target::DataBus, MicroOpCallback::BIT),
            ),
            // BMI
            OpCode::new(
                0x30,
                "BMI",
                Relative(MicroOpCallback::BRANCH(Condition::NegativeSet)),
            ),
            // BNE
            OpCode::new(
                0xD0,
                "BNE",
                Relative(MicroOpCallback::BRANCH(Condition::ZeroClear)),
            ),
            // BPL
            OpCode::new(
                0x10,
                "BPL",
                Relative(MicroOpCallback::BRANCH(Condition::NegativeClear)),
            ),
            // BRK
            OpCode::new(0x00, "BRK", BRK(MicroOpCallback::None)),
            // BVC
            OpCode::new(
                0x50,
                "BVC",
                Relative(MicroOpCallback::BRANCH(Condition::OverflowClear)),
            ),
            // BVS
            OpCode::new(
                0x70,
                "BVS",
                Relative(MicroOpCallback::BRANCH(Condition::OverflowSet)),
            ),
            // CLC
            OpCode::new(0x18, "CLC", AccumulatorOrImplied(MicroOpCallback::CLC)),
            // //CLD
            OpCode::new(0xD8, "CLD", AccumulatorOrImplied(MicroOpCallback::CLD)),
            // //CLI
            OpCode::new(0x58, "CLI", AccumulatorOrImplied(MicroOpCallback::CLI)),
            // //CLV
            OpCode::new(0xB8, "CLV", AccumulatorOrImplied(MicroOpCallback::CLV)),
            // //CMP
            OpCode::new(
                0xC9,
                "CMP",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::CMP),
            ),
            OpCode::new(
                0xC5,
                "CMP",
                ZeroPageRead(Target::DataBus, MicroOpCallback::CMP),
            ),
            OpCode::new(
                0xD5,
                "CMP",
                ZeroPageIndexRead(Source::X, Target::DataBus, MicroOpCallback::CMP),
            ),
            OpCode::new(
                0xCD,
                "CMP",
                AbsoluteRead(Target::DataBus, MicroOpCallback::CMP),
            ),
            OpCode::new(
                0xDD,
                "CMP",
                AbsoluteIndexRead(Source::X, Target::DataBus, MicroOpCallback::CMP),
            ),
            OpCode::new(
                0xD9,
                "CMP",
                AbsoluteIndexRead(Source::Y, Target::DataBus, MicroOpCallback::CMP),
            ),
            OpCode::new(
                0xC1,
                "CMP",
                IndexedIndirectRead(Target::DataBus, MicroOpCallback::CMP),
            ),
            OpCode::new(
                0xD1,
                "CMP",
                IndirectIndexedRead(Target::DataBus, MicroOpCallback::CMP),
            ),
            // //CPX
            OpCode::new(
                0xE0,
                "CPX",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::CPX),
            ),
            OpCode::new(
                0xE4,
                "CPX",
                ZeroPageRead(Target::DataBus, MicroOpCallback::CPX),
            ),
            OpCode::new(
                0xEC,
                "CPX",
                AbsoluteRead(Target::DataBus, MicroOpCallback::CPX),
            ),
            // //CPY
            OpCode::new(
                0xC0,
                "CPY",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::CPY),
            ),
            OpCode::new(
                0xC4,
                "CPY",
                ZeroPageRead(Target::DataBus, MicroOpCallback::CPY),
            ),
            OpCode::new(
                0xCC,
                "CPY",
                AbsoluteRead(Target::DataBus, MicroOpCallback::CPY),
            ),
            // //DEC
            OpCode::new(0xC6, "DEC", ZeroPageRMW(Target::DataBus, MicroOpCallback::DEC)),
            OpCode::new(
                0xD6,
                "DEC",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::DEC),
            ),
            OpCode::new(0xCE, "DEC", AbsoluteRMW(Target::DataBus, MicroOpCallback::DEC)),
            OpCode::new(
                0xDE,
                "DEC",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::DEC),
            ),
            // //DEX
            OpCode::new(0xCA, "DEX", AccumulatorOrImplied(MicroOpCallback::DEX)),
            // //DEY
            OpCode::new(0x88, "DEY", AccumulatorOrImplied(MicroOpCallback::DEY)),
            // //EOR
            OpCode::new(
                0x49,
                "EOR",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::EOR),
            ),
            OpCode::new(
                0x45,
                "EOR",
                ZeroPageRead(Target::DataBus, MicroOpCallback::EOR),
            ),
            OpCode::new(
                0x55,
                "EOR",
                ZeroPageIndexRead(Source::X, Target::DataBus, MicroOpCallback::EOR),
            ),
            OpCode::new(
                0x4D,
                "EOR",
                AbsoluteRead(Target::DataBus, MicroOpCallback::EOR),
            ),
            OpCode::new(
                0x5D,
                "EOR",
                AbsoluteIndexRead(Source::X, Target::DataBus, MicroOpCallback::EOR),
            ),
            OpCode::new(
                0x59,
                "EOR",
                AbsoluteIndexRead(Source::Y, Target::DataBus, MicroOpCallback::EOR),
            ),
            OpCode::new(
                0x41,
                "EOR",
                IndexedIndirectRead(Target::DataBus, MicroOpCallback::EOR),
            ),
            OpCode::new(
                0x51,
                "EOR",
                IndirectIndexedRead(Target::DataBus, MicroOpCallback::EOR),
            ),
            // //INC
            OpCode::new(0xE6, "INC", ZeroPageRMW(Target::DataBus, MicroOpCallback::INC)),
            OpCode::new(
                0xF6,
                "INC",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::INC),
            ),
            OpCode::new(0xEE, "INC", AbsoluteRMW(Target::DataBus, MicroOpCallback::INC)),
            OpCode::new(
                0xFE,
                "INC",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::INC),
            ),
            // //INX
            OpCode::new(0xE8, "INX", AccumulatorOrImplied(MicroOpCallback::INX)),
            // //INY
            OpCode::new(0xC8, "INY", AccumulatorOrImplied(MicroOpCallback::INY)),
            // //JMP
            OpCode::new(0x4C, "JMP", JmpAbsolute(MicroOpCallback::None)),
            OpCode::new(0x6C, "JMP", JmpIndirect(MicroOpCallback::None)),
            // //JSR
            OpCode::new(0x20, "JSR", JSR(MicroOpCallback::None)),
            // //LDA
            OpCode::new(
                0xA9,
                "LDA",
                ImmediateAddressing(Target::A, MicroOpCallback::None),
            ),
            OpCode::new(0xA5, "LDA", ZeroPageRead(Target::A, MicroOpCallback::None)),
            OpCode::new(
                0xB5,
                "LDA",
                ZeroPageIndexRead(Source::X, Target::A, MicroOpCallback::None),
            ),
            OpCode::new(0xAD, "LDA", AbsoluteRead(Target::A, MicroOpCallback::None)),
            OpCode::new(
                0xBD,
                "LDA",
                AbsoluteIndexRead(Source::X, Target::A, MicroOpCallback::None),
            ),
            OpCode::new(
                0xB9,
                "LDA",
                AbsoluteIndexRead(Source::Y, Target::A, MicroOpCallback::None),
            ),
            OpCode::new(
                0xA1,
                "LDA",
                IndexedIndirectRead(Target::A, MicroOpCallback::None),
            ),
            OpCode::new(
                0xB1,
                "LDA",
                IndirectIndexedRead(Target::A, MicroOpCallback::None),
            ),
            // //LDX
            OpCode::new(
                0xA2,
                "LDX",
                ImmediateAddressing(Target::X, MicroOpCallback::None),
            ),
            OpCode::new(0xA6, "LDX", ZeroPageRead(Target::X, MicroOpCallback::None)),
            OpCode::new(
                0xB6,
                "LDX",
                ZeroPageIndexRead(Source::Y, Target::X, MicroOpCallback::None),
            ),
            OpCode::new(0xAE, "LDX", AbsoluteRead(Target::X, MicroOpCallback::None)),
            OpCode::new(
                0xBE,
                "LDX",
                AbsoluteIndexRead(Source::Y, Target::X, MicroOpCallback::None),
            ),
            // //LDY
            OpCode::new(
                0xA0,
                "LDY",
                ImmediateAddressing(Target::Y, MicroOpCallback::None),
            ),
            OpCode::new(0xA4, "LDY", ZeroPageRead(Target::Y, MicroOpCallback::None)),
            OpCode::new(
                0xB4,
                "LDY",
                ZeroPageIndexRead(Source::X, Target::Y, MicroOpCallback::None),
            ),
            OpCode::new(0xAC, "LDY", AbsoluteRead(Target::Y, MicroOpCallback::None)),
            OpCode::new(
                0xBC,
                "LDY",
                AbsoluteIndexRead(Source::X, Target::Y, MicroOpCallback::None),
            ),
            // //LSR
            OpCode::new(0x4A, "LSR", AccumulatorOrImplied(MicroOpCallback::LSR)),
            OpCode::new(0x46, "LSR", ZeroPageRMW(Target::DataBus, MicroOpCallback::LSR)),
            OpCode::new(
                0x56,
                "LSR",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::LSR),
            ),
            OpCode::new(0x4E, "LSR", AbsoluteRMW(Target::DataBus, MicroOpCallback::LSR)),
            OpCode::new(
                0x5E,
                "LSR",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::LSR),
            ),
            // //NOP
            OpCode::new(0xEA, "NOP", AccumulatorOrImplied(MicroOpCallback::None)),
            // //ORA
            OpCode::new(
                0x09,
                "ORA",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::ORA),
            ),
            OpCode::new(
                0x05,
                "ORA",
                ZeroPageRead(Target::DataBus, MicroOpCallback::ORA),
            ),
            OpCode::new(
                0x15,
                "ORA",
                ZeroPageIndexRead(Source::X, Target::DataBus, MicroOpCallback::ORA),
            ),
            OpCode::new(
                0x0D,
                "ORA",
                AbsoluteRead(Target::DataBus, MicroOpCallback::ORA),
            ),
            OpCode::new(
                0x1D,
                "ORA",
                AbsoluteIndexRead(Source::X, Target::DataBus, MicroOpCallback::ORA),
            ),
            OpCode::new(
                0x19,
                "ORA",
                AbsoluteIndexRead(Source::Y, Target::DataBus, MicroOpCallback::ORA),
            ),
            OpCode::new(
                0x01,
                "ORA",
                IndexedIndirectRead(Target::DataBus, MicroOpCallback::ORA),
            ),
            OpCode::new(
                0x11,
                "ORA",
                IndirectIndexedRead(Target::DataBus, MicroOpCallback::ORA),
            ),
            // //PHA
            OpCode::new(0x48, "PHA", PH(Source::A, MicroOpCallback::None)),
            // //PHP
            OpCode::new(0x08, "PHP", PH(Source::PBrk, MicroOpCallback::None)),
            // //PLA
            OpCode::new(0x68, "PLA", PL(Target::A, MicroOpCallback::None)),
            // //PLP
            OpCode::new(0x28, "PLP", PL(Target::P, MicroOpCallback::None)),
            // //ROL
            OpCode::new(0x2A, "ROL", AccumulatorOrImplied(MicroOpCallback::ROL)),
            OpCode::new(0x26, "ROL", ZeroPageRMW(Target::DataBus, MicroOpCallback::ROL)),
            OpCode::new(
                0x36,
                "ROL",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::ROL),
            ),
            OpCode::new(0x2E, "ROL", AbsoluteRMW(Target::DataBus, MicroOpCallback::ROL)),
            OpCode::new(
                0x3E,
                "ROL",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::ROL),
            ),
            // //ROR
            OpCode::new(0x6A, "ROR", AccumulatorOrImplied(MicroOpCallback::ROR)),
            OpCode::new(0x66, "ROR", ZeroPageRMW(Target::DataBus, MicroOpCallback::ROR)),
            OpCode::new(
                0x76,
                "ROR",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::ROR),
            ),
            OpCode::new(0x6E, "ROR", AbsoluteRMW(Target::DataBus, MicroOpCallback::ROR)),
            OpCode::new(
                0x7E,
                "ROR",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::ROR),
            ),
            // //RTI
            OpCode::new(0x40, "RTI", RTI(MicroOpCallback::None)),
            // //RTS
            OpCode::new(0x60, "RTS", RTS(MicroOpCallback::None)),
            // //SBC
            OpCode::new(
                0xE9,
                "SBC",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::SBC),
            ),
            OpCode::new(
                0xE5,
                "SBC",
                ZeroPageRead(Target::DataBus, MicroOpCallback::SBC),
            ),
            OpCode::new(
                0xF5,
                "SBC",
                ZeroPageIndexRead(Source::X, Target::DataBus, MicroOpCallback::SBC),
            ),
            OpCode::new(
                0xED,
                "SBC",
                AbsoluteRead(Target::DataBus, MicroOpCallback::SBC),
            ),
            OpCode::new(
                0xFD,
                "SBC",
                AbsoluteIndexRead(Source::X, Target::DataBus, MicroOpCallback::SBC),
            ),
            OpCode::new(
                0xF9,
                "SBC",
                AbsoluteIndexRead(Source::Y, Target::DataBus, MicroOpCallback::SBC),
            ),
            OpCode::new(
                0xE1,
                "SBC",
                IndexedIndirectRead(Target::DataBus, MicroOpCallback::SBC),
            ),
            OpCode::new(
                0xF1,
                "SBC",
                IndirectIndexedRead(Target::DataBus, MicroOpCallback::SBC),
            ),
            // //SEC
            OpCode::new(0x38, "SEC", AccumulatorOrImplied(MicroOpCallback::SEC)),
            // //SED
            OpCode::new(0xF8, "SED", AccumulatorOrImplied(MicroOpCallback::SED)),
            // //SEI
            OpCode::new(0x78, "SEI", AccumulatorOrImplied(MicroOpCallback::SEI)),
            // //STA
            OpCode::new(0x85, "STA", ZeroPageWrite(Source::A, MicroOpCallback::None)),
            OpCode::new(
                0x95,
                "STA",
                ZeroPageIndexWrite(Source::A, Source::X, MicroOpCallback::None),
            ),
            OpCode::new(0x8D, "STA", AbsoluteWrite(Source::A, MicroOpCallback::None)),
            OpCode::new(
                0x9D,
                "STA",
                AbsoluteIndexWrite(Source::A, Source::X, MicroOpCallback::None),
            ),
            OpCode::new(
                0x99,
                "STA",
                AbsoluteIndexWrite(Source::A, Source::Y, MicroOpCallback::None),
            ),
            OpCode::new(
                0x81,
                "STA",
                IndexedIndirectWrite(Source::A, MicroOpCallback::None),
            ),
            OpCode::new(
                0x91,
                "STA",
                IndirectIndexedWrite(Source::A, MicroOpCallback::None),
            ),
            // //STX
            OpCode::new(0x86, "STX", ZeroPageWrite(Source::X, MicroOpCallback::None)),
            OpCode::new(
                0x96,
                "STX",
                ZeroPageIndexWrite(Source::X, Source::Y, MicroOpCallback::None),
            ),
            OpCode::new(0x8E, "STX", AbsoluteWrite(Source::X, MicroOpCallback::None)),
            // //STY
            OpCode::new(0x84, "STY", ZeroPageWrite(Source::Y, MicroOpCallback::None)),
            OpCode::new(
                0x94,
                "STY",
                ZeroPageIndexWrite(Source::Y, Source::X, MicroOpCallback::None),
            ),
            OpCode::new(0x8C, "STY", AbsoluteWrite(Source::Y, MicroOpCallback::None)),
            // //TAX
            OpCode::new(0xAA, "TAX", AccumulatorOrImplied(MicroOpCallback::TAX)),
            // //TAY
            OpCode::new(0xA8, "TAY", AccumulatorOrImplied(MicroOpCallback::TAY)),
            // //TSX
            OpCode::new(0xBA, "TSX", AccumulatorOrImplied(MicroOpCallback::TSX)),
            // //TXA
            OpCode::new(0x8A, "TXA", AccumulatorOrImplied(MicroOpCallback::TXA)),
            // //TXS
            OpCode::new(0x9A, "TXS", AccumulatorOrImplied(MicroOpCallback::TXS)),
            // //TYA
            OpCode::new(0x98, "TYA", AccumulatorOrImplied(MicroOpCallback::TYA)),
            // //Illegal Opcodes
            OpCode::new(0x1A, "*NOP", AccumulatorOrImplied(MicroOpCallback::None)),
            OpCode::new(0x3A, "*NOP", AccumulatorOrImplied(MicroOpCallback::None)),
            OpCode::new(0x5A, "*NOP", AccumulatorOrImplied(MicroOpCallback::None)),
            OpCode::new(0x7A, "*NOP", AccumulatorOrImplied(MicroOpCallback::None)),
            OpCode::new(0xDA, "*NOP", AccumulatorOrImplied(MicroOpCallback::None)),
            OpCode::new(0xFA, "*NOP", AccumulatorOrImplied(MicroOpCallback::None)),
            OpCode::new(
                0x80,
                "*NOP",
                ImmediateAddressing(Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x82,
                "*NOP",
                ImmediateAddressing(Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x89,
                "*NOP",
                ImmediateAddressing(Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0xC2,
                "*NOP",
                ImmediateAddressing(Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0xE2,
                "*NOP",
                ImmediateAddressing(Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x04,
                "*NOP",
                ZeroPageRead(Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x44,
                "*NOP",
                ZeroPageRead(Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x64,
                "*NOP",
                ZeroPageRead(Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x14,
                "*NOP",
                ZeroPageIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x34,
                "*NOP",
                ZeroPageIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x54,
                "*NOP",
                ZeroPageIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x74,
                "*NOP",
                ZeroPageIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0xD4,
                "*NOP",
                ZeroPageIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0xF4,
                "*NOP",
                ZeroPageIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x0C,
                "*NOP",
                AbsoluteRead(Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x1C,
                "*NOP",
                AbsoluteIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x3C,
                "*NOP",
                AbsoluteIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x5C,
                "*NOP",
                AbsoluteIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x7C,
                "*NOP",
                AbsoluteIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0xDC,
                "*NOP",
                AbsoluteIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0xFC,
                "*NOP",
                AbsoluteIndexRead(Source::X, Target::None, MicroOpCallback::None),
            ),
            OpCode::new(
                0x4B,
                "*ALR",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::ALR),
            ),
            OpCode::new(
                0x0B,
                "*ANC",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::ANC),
            ),
            OpCode::new(
                0x2B,
                "*ANC",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::ANC2),
            ),
            OpCode::new(
                0x8B,
                "*ANE",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::ANE),
            ),
            OpCode::new(
                0x6B,
                "*ARR",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::ARR),
            ),
            OpCode::new(
                0xC7,
                "*DCP",
                ZeroPageRMW(Target::DataBus, MicroOpCallback::DCP),
            ),
            OpCode::new(
                0xD7,
                "*DCP",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::DCP),
            ),
            OpCode::new(
                0xCF,
                "*DCP",
                AbsoluteRMW(Target::DataBus, MicroOpCallback::DCP),
            ),
            OpCode::new(
                0xDF,
                "*DCP",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::DCP),
            ),
            OpCode::new(
                0xDB,
                "*DCP",
                AbsoluteIndexRMW(Source::Y, MicroOpCallback::DCP),
            ),
            OpCode::new(0xC3, "*DCP", IndexedIndirectRMW(MicroOpCallback::DCP)),
            OpCode::new(0xD3, "*DCP", IndirectIndexedRMW(MicroOpCallback::DCP)),
            OpCode::new(
                0xE7,
                "*ISB",
                ZeroPageRMW(Target::DataBus, MicroOpCallback::ISB),
            ),
            OpCode::new(
                0xF7,
                "*ISB",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::ISB),
            ),
            OpCode::new(
                0xEF,
                "*ISB",
                AbsoluteRMW(Target::DataBus, MicroOpCallback::ISB),
            ),
            OpCode::new(
                0xFF,
                "*ISB",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::ISB),
            ),
            OpCode::new(
                0xFB,
                "*ISB",
                AbsoluteIndexRMW(Source::Y, MicroOpCallback::ISB),
            ),
            OpCode::new(0xE3, "*ISB", IndexedIndirectRMW(MicroOpCallback::ISB)),
            OpCode::new(0xF3, "*ISB", IndirectIndexedRMW(MicroOpCallback::ISB)),
            OpCode::new(
                0xBB,
                "*LAS",
                AbsoluteIndexRead(Source::Y, Target::DataBus, MicroOpCallback::LAS),
            ),
            OpCode::new(
                0xA7,
                "*LAX",
                ZeroPageRead(Target::DataBus, MicroOpCallback::LAX),
            ),
            OpCode::new(
                0xB7,
                "*LAX",
                ZeroPageIndexRead(Source::Y, Target::DataBus, MicroOpCallback::LAX),
            ),
            OpCode::new(
                0xAF,
                "*LAX",
                AbsoluteRead(Target::DataBus, MicroOpCallback::LAX),
            ),
            OpCode::new(
                0xBF,
                "*LAX",
                AbsoluteIndexRead(Source::Y, Target::DataBus, MicroOpCallback::LAX),
            ),
            OpCode::new(
                0xA3,
                "*LAX",
                IndexedIndirectRead(Target::DataBus, MicroOpCallback::LAX),
            ),
            OpCode::new(
                0xB3,
                "*LAX",
                IndirectIndexedRead(Target::DataBus, MicroOpCallback::LAX),
            ),
            OpCode::new(
                0xAB,
                "*LXA",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::LXA),
            ),
            OpCode::new(
                0x27,
                "*RLA",
                ZeroPageRMW(Target::DataBus, MicroOpCallback::RLA),
            ),
            OpCode::new(
                0x37,
                "*RLA",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::RLA),
            ),
            OpCode::new(
                0x2F,
                "*RLA",
                AbsoluteRMW(Target::DataBus, MicroOpCallback::RLA),
            ),
            OpCode::new(
                0x3F,
                "*RLA",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::RLA),
            ),
            OpCode::new(
                0x3B,
                "*RLA",
                AbsoluteIndexRMW(Source::Y, MicroOpCallback::RLA),
            ),
            OpCode::new(0x23, "*RLA", IndexedIndirectRMW(MicroOpCallback::RLA)),
            OpCode::new(0x33, "*RLA", IndirectIndexedRMW(MicroOpCallback::RLA)),
            OpCode::new(
                0x67,
                "*RRA",
                ZeroPageRMW(Target::DataBus, MicroOpCallback::RRA),
            ),
            OpCode::new(
                0x77,
                "*RRA",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::RRA),
            ),
            OpCode::new(
                0x6F,
                "*RRA",
                AbsoluteRMW(Target::DataBus, MicroOpCallback::RRA),
            ),
            OpCode::new(
                0x7F,
                "*RRA",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::RRA),
            ),
            OpCode::new(
                0x7B,
                "*RRA",
                AbsoluteIndexRMW(Source::Y, MicroOpCallback::RRA),
            ),
            OpCode::new(0x63, "*RRA", IndexedIndirectRMW(MicroOpCallback::RRA)),
            OpCode::new(0x73, "*RRA", IndirectIndexedRMW(MicroOpCallback::RRA)),
            OpCode::new(
                0x87,
                "*SAX",
                ZeroPageWrite(Source::DataBus, MicroOpCallback::SAX),
            ),
            OpCode::new(
                0x97,
                "*SAX",
                ZeroPageIndexWrite(Source::DataBus, Source::Y, MicroOpCallback::SAX),
            ),
            OpCode::new(
                0x8F,
                "*SAX",
                AbsoluteWrite(Source::DataBus, MicroOpCallback::SAX),
            ),
            OpCode::new(
                0x83,
                "*SAX",
                IndexedIndirectWrite(Source::DataBus, MicroOpCallback::SAX),
            ),
            OpCode::new(
                0xCB,
                "*SBX",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::SBX),
            ),
            OpCode::new(
                0x9F,
                "*SHA",
                AbsoluteIndexWrite(Source::DataBus, Source::Y, MicroOpCallback::SHA),
            ),
            OpCode::new(
                0x93,
                "*SHA",
                IndirectIndexedWrite(Source::DataBus, MicroOpCallback::SHA),
            ),
            OpCode::new(
                0x9E,
                "*SHX",
                AbsoluteIndexWrite(Source::DataBus, Source::Y, MicroOpCallback::SHX),
            ),
            OpCode::new(
                0x9C,
                "*SHY",
                AbsoluteIndexWrite(Source::DataBus, Source::X, MicroOpCallback::SHY),
            ),
            OpCode::new(
                0x07,
                "*SLO",
                ZeroPageRMW(Target::DataBus, MicroOpCallback::SLO),
            ),
            OpCode::new(
                0x17,
                "*SLO",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::SLO),
            ),
            OpCode::new(
                0x0F,
                "*SLO",
                AbsoluteRMW(Target::DataBus, MicroOpCallback::SLO),
            ),
            OpCode::new(
                0x1F,
                "*SLO",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::SLO),
            ),
            OpCode::new(
                0x1B,
                "*SLO",
                AbsoluteIndexRMW(Source::Y, MicroOpCallback::SLO),
            ),
            OpCode::new(0x03, "*SLO", IndexedIndirectRMW(MicroOpCallback::SLO)),
            OpCode::new(0x13, "*SLO", IndirectIndexedRMW(MicroOpCallback::SLO)),
            OpCode::new(
                0x47,
                "*SRE",
                ZeroPageRMW(Target::DataBus, MicroOpCallback::SRE),
            ),
            OpCode::new(
                0x57,
                "*SRE",
                ZeroPageIndexRMW(Source::X, MicroOpCallback::SRE),
            ),
            OpCode::new(
                0x4F,
                "*SRE",
                AbsoluteRMW(Target::DataBus, MicroOpCallback::SRE),
            ),
            OpCode::new(
                0x5F,
                "*SRE",
                AbsoluteIndexRMW(Source::X, MicroOpCallback::SRE),
            ),
            OpCode::new(
                0x5B,
                "*SRE",
                AbsoluteIndexRMW(Source::Y, MicroOpCallback::SRE),
            ),
            OpCode::new(0x43, "*SRE", IndexedIndirectRMW(MicroOpCallback::SRE)),
            OpCode::new(0x53, "*SRE", IndirectIndexedRMW(MicroOpCallback::SRE)),
            OpCode::new(
                0x9B,
                "*TAS",
                AbsoluteIndexRead(Source::X, Target::DataBus, MicroOpCallback::TAS),
            ),
            OpCode::new(
                0xEB,
                "*SBC",
                ImmediateAddressing(Target::DataBus, MicroOpCallback::SBC),
            ),
            OpCode::new(
                0x02,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0x12,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0x22,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0x32,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0x42,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0x52,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0x62,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0x72,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0x92,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0xB2,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0xD2,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
            OpCode::new(
                0xF2,
                "*JAM",
                ImmediateAddressing(Target::None, MicroOpCallback::JAM),
            ),
        ])
        .ok();

    let mut map = HashMap::new();

    for opcode in OPCODES.get().unwrap() {
        map.insert(opcode.opcode, opcode);
    }

    map
}

/// Initialize the fast lookup table - call once during setup.
/// Note: OPCODES_MAP.get_or_init(init) is required because init() populates the OPCODES vector.
pub fn init_lookup_table() -> [Option<OpCode>; 256] {
    // Ensure OPCODES is initialized (init() populates both OPCODES and returns HashMap)
    OPCODES_MAP.get_or_init(init);

    let mut table: [Option<OpCode>; 256] = [None; 256];
    for opcode in OPCODES.get().unwrap() {
        table[opcode.opcode as usize] = Some(*opcode);
    }
    table
}

/// Fast opcode lookup - O(1) array access
#[inline]
pub fn get_opcode(opcode: u8) -> Option<OpCode> {
    OPCODES_TABLE.get_or_init(init_lookup_table)[opcode as usize]
}

impl OpCode {
    pub fn new(opcode: u8, name: &'static str, op_type: OpType) -> Self {
        Self {
            opcode,
            name,
            op_type,
        }
    }
}

pub fn get_bytes_for_opcode(op: OpCode) -> u8 {
    match op.op_type {
        ImmediateAddressing(..) => 1,
        AbsoluteRead(..) => 2,
        AbsoluteIndexRead(..) => 2,
        ZeroPageRead(..) => 1,
        ZeroPageIndexRead(..) => 1,
        AccumulatorOrImplied(_) => 0,
        IndexedIndirectRead(..) => 1,
        IndirectIndexedRead(..) => 1,
        BRK(_) => 0,
        RTI(_) => 0,
        RTS(_) => 0,
        PH(..) => 0,
        PL(..) => 0,
        JSR(_) => 2,
        JmpAbsolute(_) => 2,
        AbsoluteRMW(..) => 2,
        AbsoluteWrite(..) => 2,
        ZeroPageRMW(..) => 1,
        ZeroPageWrite(..) => 1,
        ZeroPageIndexRMW(..) => 1,
        ZeroPageIndexWrite(..) => 1,
        AbsoluteIndexRMW(..) => 2,
        AbsoluteIndexWrite(..) => 2,
        IndexedIndirectWrite(..) => 1,
        JmpIndirect(_) => 2,
        IndirectIndexedWrite(..) => 1,
        Relative(_) => 1,
        IndexedIndirectRMW(_) => 1,
        IndirectIndexedRMW(_) => 1,
    }
}
