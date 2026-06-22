use crate::emulation::nes::{MASTER_CYCLES_PER_FRAME, Nes};
use crate::util::{parse_hex_u8, parse_hex_u16};

/// A stop condition that can be checked during execution
#[derive(Debug, Clone)]
pub enum StopCondition {
    /// Stop after N master cycles
    Cycles(u64),
    /// Stop after N frames
    Frames(u64),
    /// Stop when PC reaches address (breakpoint)
    PcEquals(u16),
    /// Stop when opcode is executed
    Opcode(u8),
    /// Stop when memory address equals value
    MemoryEquals {
        addr: u16,
        value: u8,
        and: Option<Box<StopCondition>>,
    },
    /// Stop when memory address does not equal value
    MemoryNotEquals {
        addr: u16,
        value: u8,
        and: Option<Box<StopCondition>>,
    },
    /// Stop on HLT instruction
    OnHalt,
    /// Breakpoint at address (alias for PcEquals, kept for backward
    /// compatibility)
    Breakpoint(u16),
    /// Watch memory address for access
    MemoryWatch {
        addr: u16,
        access_type: MemoryAccessType,
    },
}

impl StopCondition {
    /// Parse a memory condition string like "0x6000==0x80" or "0x6000!=0x00"
    pub fn parse_memory_condition(vec: &Vec<String>) -> Result<Vec<Self>, String> {
        let mut res = Vec::new();
        for s in vec {
            let cond = Self::parse_single_condition(s);

            #[allow(clippy::question_mark)]
            if let Ok(cond) = cond {
                res.push(cond)
            } else if let Err(cond) = cond {
                return Err(cond);
            }
        }

        Ok(res)
    }

    pub fn parse_single_condition(s: &String) -> Result<Self, String> {
        if let Some((cond1, cond2)) = s.split_once("&&") {
            let cond1 = Self::parse_single_condition(&cond1.to_string());
            let cond2 = Self::parse_single_condition(&cond2.to_string());

            if let (Ok(cond1), Ok(cond2)) = (cond1, cond2) {
                match cond1 {
                    StopCondition::MemoryEquals {
                        addr,
                        value,
                        ..
                    } => {
                        return Ok(StopCondition::MemoryEquals {
                            addr,
                            value,
                            and: Some(Box::new(cond2)),
                        });
                    }
                    StopCondition::MemoryNotEquals {
                        addr,
                        value,
                        ..
                    } => {
                        return Ok(StopCondition::MemoryNotEquals {
                            addr,
                            value,
                            and: Some(Box::new(cond2)),
                        });
                    }
                    _ => {}
                }
            }
        }

        if let Some((addr_str, val_str)) = s.split_once("==") {
            let addr = parse_hex_u16(addr_str.trim())?;
            let value = parse_hex_u8(val_str.trim())?;
            Ok(StopCondition::MemoryEquals {
                addr,
                value,
                and: None,
            })
        } else if let Some((addr_str, val_str)) = s.split_once("!=") {
            let addr = parse_hex_u16(addr_str.trim())?;
            let value = parse_hex_u8(val_str.trim())?;
            Ok(StopCondition::MemoryNotEquals {
                addr,
                value,
                and: None,
            })
        } else {
            Err(format!(
                "Invalid memory condition '{}'. Expected format: ADDR==VALUE or ADDR!=VALUE",
                s
            ))
        }
    }

    /// Parse a memory watch string like "0x2002" or "0x2002:r" or "0x4016:w"
    pub fn parse_memory_watch(s: &str) -> Result<Self, String> {
        let (addr_str, access_type) = if let Some((addr_part, mode_part)) = s.split_once(':') {
            (addr_part, MemoryAccessType::parse(mode_part)?)
        } else {
            (s, MemoryAccessType::ReadWrite) // Default to both reads and writes
        };

        let addr = parse_hex_u16(addr_str.trim())?;
        Ok(StopCondition::MemoryWatch {
            addr,
            access_type,
        })
    }

    /// Parse multiple memory watch conditions
    pub fn parse_memory_watches(watches: &[String]) -> Result<Vec<Self>, String> {
        watches
            .iter()
            .map(|s| Self::parse_memory_watch(s))
            .collect()
    }

    pub fn check(&self, emu: &mut Nes) -> bool {
        match self {
            StopCondition::Cycles(target) => emu.total_cycles >= *target,
            StopCondition::Frames(target) => {
                emu.total_cycles >= *target * MASTER_CYCLES_PER_FRAME as u64
            }
            StopCondition::PcEquals(addr) | StopCondition::Breakpoint(addr) => {
                emu.program_counter() == *addr
            }
            StopCondition::Opcode(op) => emu.current_opcode_byte() == *op,
            StopCondition::MemoryEquals {
                addr,
                value,
                and,
            } => {
                let and = and.as_ref().map(|and| and.check(emu));

                let mem_val = emu.get_memory_debug(Some(*addr..=*addr))[0]
                    .first()
                    .copied()
                    .unwrap_or(0);

                if let Some(and) = and {
                    mem_val == *value && and
                } else {
                    mem_val == *value
                }
            }
            StopCondition::MemoryNotEquals {
                addr,
                value,
                and,
            } => {
                let and = and.as_ref().map(|and| and.check(emu));

                let mem_val = emu.get_memory_debug(Some(*addr..=*addr))[0]
                    .first()
                    .copied()
                    .unwrap_or(0);

                if let Some(and) = and {
                    mem_val != *value && and
                } else {
                    mem_val != *value
                }
            }
            StopCondition::OnHalt => emu.is_halted(),
            StopCondition::MemoryWatch {
                addr,
                access_type,
            } => {
                // Check if CPU accessed this address
                if let Some(last_access) = emu.last_memory_access() {
                    let (access_addr, was_read, _) = last_access;
                    if access_addr == *addr {
                        match access_type {
                            MemoryAccessType::Read => was_read,
                            MemoryAccessType::Write => !was_read,
                            MemoryAccessType::ReadWrite => true,
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }

    pub fn reason(&self, emu: &mut Nes) -> StopReason {
        match self {
            StopCondition::Cycles(_) => StopReason::CyclesReached(emu.total_cycles),
            StopCondition::Frames(_) => {
                StopReason::FramesReached(emu.total_cycles / MASTER_CYCLES_PER_FRAME as u64)
            }
            StopCondition::PcEquals(addr) | StopCondition::Breakpoint(addr) => {
                StopReason::PcReached(*addr)
            }
            StopCondition::Opcode(_) => StopReason::PcReached(emu.program_counter()),
            StopCondition::MemoryEquals {
                addr, ..
            }
            | StopCondition::MemoryNotEquals {
                addr, ..
            } => {
                let mem_val = emu.get_memory_debug(Some(*addr..=*addr))[0]
                    .first()
                    .copied()
                    .unwrap_or(0);

                StopReason::MemoryCondition(*addr, mem_val)
            }
            StopCondition::OnHalt => StopReason::Halted,
            StopCondition::MemoryWatch {
                addr,
                access_type,
            } => {
                let was_read = emu
                    .last_memory_access()
                    .map(|(_, was_read, _)| was_read)
                    .unwrap_or(true);
                StopReason::MemoryWatchpoint {
                    addr: *addr,
                    access_type: *access_type,
                    was_read,
                }
            }
        }
    }
}

/// Reason why execution stopped
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Reached target cycle count
    CyclesReached(u64),
    /// Reached target frame count
    FramesReached(u64),
    /// PC reached target address (breakpoint)
    PcReached(u16),
    /// Memory condition was met
    MemoryCondition(u16, u8),
    /// Memory watchpoint triggered
    MemoryWatchpoint {
        addr: u16,
        access_type: MemoryAccessType,
        was_read: bool,
    },
    /// HLT (illegal halt) instruction executed
    Halted,
    /// User-requested stop (e.g., breakpoint)
    Breakpoint(u16),
    /// Execution error occurred
    Error(String),
}

/// Memory access type for watchpoints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAccessType {
    /// Watch for reads only
    Read,
    /// Watch for writes only
    Write,
    /// Watch for both reads and writes
    ReadWrite,
}

impl MemoryAccessType {
    /// Parse access type from string (r, w, rw)
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "r" | "read" => Ok(Self::Read),
            "w" | "write" => Ok(Self::Write),
            "rw" | "readwrite" | "both" => Ok(Self::ReadWrite),
            _ => Err(format!(
                "Invalid memory access type '{}'. Expected: r, w, or rw",
                s
            )),
        }
    }
}
