use crate::emulation::mem::{Memory, MemoryDevice};

#[derive(Debug, Clone)]
pub struct MirrorMemory {
    base: Box<Memory>,
    mirror_mask: u16,
}

impl MirrorMemory {
    pub fn new(base: Box<Memory>, mirror_mask: u16) -> Self {
        Self {
            base,
            mirror_mask,
        }
    }
}

impl MemoryDevice for MirrorMemory {
    #[inline]
    fn read(&self, addr: u16, open_bus: u8) -> u8 {
        self.base.read(addr & self.mirror_mask, open_bus)
    }

    #[inline]
    fn write(&mut self, addr: u16, data: u8) { self.base.write(addr & self.mirror_mask, data) }

    #[inline]
    fn init(&mut self, addr: u16, data: u8) { self.base.init(addr & self.mirror_mask, data) }

    fn load(&mut self, data: Box<[u8]>) { self.base.load(data) }

    fn snapshot(&self, addr: u16, open_bus: u8) -> u8 {
        self.base.snapshot(addr & self.mirror_mask, open_bus)
    }

    fn snapshot_all(&self) -> Vec<u8> { self.base.snapshot_all() }
}
