use serde::{Deserialize, Serialize};

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
    pub fn resolve_address(self, address: u16) -> u16 {
        let address = address & 0x0FFF;
        let table = address >> 10;
        let offset = address & 0x03FF;

        match self {
            NametableArrangement::Horizontal => {
                if table & 1 == 0 {
                    offset
                } else {
                    0x400 + offset
                }
            }

            NametableArrangement::Vertical => {
                if table < 2 {
                    offset
                } else {
                    0x400 + offset
                }
            }

            NametableArrangement::SingleScreenLower => offset,
            NametableArrangement::SingleScreenUpper => 0x400 + offset,
            NametableArrangement::FourScreen => address,
        }
    }
}
