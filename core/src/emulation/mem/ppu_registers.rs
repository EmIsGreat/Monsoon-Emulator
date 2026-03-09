use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

use crate::emulation::mem::MemoryDevice;
use crate::emulation::ppu::{PALETTE_RAM_END_INDEX, PALETTE_RAM_START_ADDRESS, Ppu};

#[derive(Clone)]
pub struct PpuRegisters {
    ppu: Rc<RefCell<Ppu>>,
}

impl PpuRegisters {
    pub fn new(ppu: Rc<RefCell<Ppu>>) -> Self {
        Self {
            ppu,
        }
    }
}

impl Debug for PpuRegisters {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entry(&"0x0", &self.snapshot(0, 0))
            .entry(&"0x2", &self.snapshot(2, 0))
            .entry(&"0x4", &self.snapshot(4, 0))
            .entry(&"0x7", &self.snapshot(7, 0))
            .finish()
    }
}

impl MemoryDevice for PpuRegisters {
    #[inline(always)]
    fn read(&self, addr: u16, _: u8) -> u8 {
        match addr {
            0x2 => {
                let ppu = self.ppu.borrow();
                let val = ppu.get_ppu_status();

                let mut bus = ppu.open_bus.get();
                bus.set_masked(val, 0b1110_0000);
                ppu.open_bus.set(bus);
                bus.read()
            }
            0x4 => {
                let mut ppu = self.ppu.borrow_mut();
                let val = ppu.get_oam_at_addr();
                let mut bus = ppu.open_bus.get();
                bus.set_masked(val, 0xFF);
                ppu.open_bus.set(bus);
                bus.read()
            }
            0x7 => {
                let mut ppu = self.ppu.borrow_mut();
                let val = ppu.get_vram_at_addr();

                let mut bus = ppu.open_bus.get();
                match ppu.v_register {
                    PALETTE_RAM_START_ADDRESS..=PALETTE_RAM_END_INDEX => {
                        bus.set_masked(val, 0b0011_1111);
                    }
                    _ => {
                        bus.set_masked(val, 0xFF);
                    }
                }
                ppu.open_bus.set(bus);
                bus.read()
            }
            _ => self.ppu.borrow().open_bus.get().read(),
        }
    }

    #[inline(always)]
    fn write(&mut self, addr: u16, data: u8) {
        let mut ppu = self.ppu.borrow_mut();
        let mut bus = ppu.open_bus.get();
        bus.set_masked(data, 0xFF);
        match addr {
            0x0 => {
                ppu.set_ppu_ctrl(data);
            }
            0x1 => {
                ppu.set_mask_register(data);
            }
            0x3 => {
                ppu.set_oam_addr_register(data);
            }
            0x4 => {
                ppu.write_oam(data);
            }
            0x5 => {
                ppu.write_ppu_scroll(data);
            }
            0x6 => {
                ppu.write_vram_addr(data);
            }
            0x7 => {
                ppu.write_vram(data);
            }
            _ => (),
        };

        ppu.open_bus.set(bus);
    }

    #[inline]
    fn init(&mut self, _: u16, _: u8) {}

    #[inline]
    fn load(&mut self, _: Box<[u8]>) {}

    #[inline]
    fn snapshot(&self, addr: u16, _: u8) -> u8 {
        match addr {
            0x0 => {
                let ppu = self.ppu.borrow();
                ppu.get_ppu_ctrl()
            }
            0x2 => {
                let ppu = self.ppu.borrow();
                ppu.status_register.get()
            }
            0x4 => {
                let mut ppu = self.ppu.borrow_mut();
                ppu.get_oam_at_addr()
            }
            0x7 => {
                let ppu = self.ppu.borrow();
                ppu.ppu_data_buffer
            }
            _ => 0,
        }
    }

    #[inline]
    fn snapshot_all(&self) -> Vec<u8> {
        vec![
            self.snapshot(0, 0),
            self.snapshot(2, 0),
            self.snapshot(4, 0),
            self.snapshot(7, 0),
        ]
    }
}
