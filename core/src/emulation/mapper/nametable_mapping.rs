use serde::{Deserialize, Serialize};

use crate::emulation::ppu::NAMETABLE_SIZE;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NametableArrangement {
    Vertical,
    Horizontal,
    SingleScreenLower,
    SingleScreenUpper,
    FourScreen,
}

impl NametableArrangement {
    #[inline]
    pub fn resolve_address(&self, address: u16) -> u16 {
        let address = address - 0x2000;

        let table = address / NAMETABLE_SIZE;
        let offset = address % NAMETABLE_SIZE;

        match self {
            NametableArrangement::Horizontal => match table {
                0 | 2 => offset,
                1 | 3 => 0x400 + offset,
                _ => {
                    dbg!();
                    panic!()
                }
            },
            NametableArrangement::Vertical => match table {
                0 | 1 => offset,
                2 | 3 => 0x400 + offset,
                _ => {
                    dbg!();
                    panic!()
                }
            },
            NametableArrangement::SingleScreenLower => offset,
            NametableArrangement::SingleScreenUpper => 0x400 + offset,
            NametableArrangement::FourScreen => address,
        }
    }
}
