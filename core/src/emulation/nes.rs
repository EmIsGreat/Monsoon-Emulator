use std::cell::RefCell;
use std::fmt::Debug;
use std::ops::{Deref, RangeInclusive};
use std::rc::Rc;
use std::time::Duration;

use crate::emulation::cpu::{Cpu, MicroOp};
use crate::emulation::mem::Memory;
use crate::emulation::mem::mirror_memory::MirrorMemory;
use crate::emulation::mem::ppu_registers::PpuRegisters;
use crate::emulation::ppu::Ppu;
use crate::emulation::rom::RomFile;
use crate::emulation::savestate::{CpuState, PpuState, SaveState};
use crate::trace::TraceLog;

pub const CPU_CYCLES_PER_FRAME: u16 = 29780;
pub const FRAME_DURATION: Duration = Duration::from_nanos(16_666_667);
pub const MASTER_CYCLES_PER_FRAME: u32 = 357366;

pub struct Nes {
    pub cpu: Cpu,
    pub ppu: Rc<RefCell<Ppu>>,
    pub total_cycles: u128,
    pub rom_file: Option<RomFile>,
    pub trace_log: Option<TraceLog>,
    pub cpu_cycle_counter: u8,
    pub ppu_cycle_counter: u8,
}

impl Nes {
    #[inline]
    pub fn get_pixel_buffer(&self) -> Vec<u16> { self.ppu.borrow().pixel_buffer.clone() }

    pub fn power(&mut self) {
        self.cpu.ppu = Some(self.ppu.clone());

        self.cpu.memory.add_memory(
            0x2000..=0x3FFF,
            Memory::MirrorMemory(MirrorMemory::new(
                Box::new(Memory::PpuRegisters(PpuRegisters::new(self.ppu.clone()))),
                0x0007,
            )),
        );

        self.cpu.reset();
    }

    pub fn power_off(&mut self) {
        self.cpu = Cpu::default();
        self.ppu = Rc::new(RefCell::new(Ppu::default()));
        self.total_cycles = 0;
        self.cpu_cycle_counter = 0;
        self.ppu_cycle_counter = 0;
    }

    pub fn run(&mut self) -> Result<ExecutionFinishedType, String> {
        self.run_until(u128::MAX, false)
    }

    pub fn run_until(
        &mut self,
        last_cycle: u128,
        stop_at_frame: bool,
    ) -> Result<ExecutionFinishedType, String> {
        loop {
            let res = self.step_internal(last_cycle, stop_at_frame);
            match res {
                Ok(ExecutionFinishedType::CycleCompleted) => {
                    continue;
                }
                Ok(res) => {
                    return Ok(res);
                }
                Err(err) => {
                    panic!("{}", err)
                }
            };
        }
    }

    pub fn get_memory_debug(&self, range: Option<RangeInclusive<u16>>) -> Vec<Vec<u8>> {
        vec![
            self.cpu.get_memory_debug(range.clone()),
            self.ppu.borrow().get_memory_debug(range.clone()),
        ]
    }

    #[inline]
    pub fn step_frame(&mut self) -> Result<ExecutionFinishedType, String> {
        self.run_until(u128::MAX, true)
    }
}

impl Nes {
    pub fn new(cpu: Cpu, ppu: Rc<RefCell<Ppu>>) -> Self {
        Self {
            cpu,
            ppu,
            rom_file: None,
            trace_log: None,
            total_cycles: 0,
            cpu_cycle_counter: 0,
            ppu_cycle_counter: 0,
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
        self.ppu.borrow_mut().reset();
    }

    pub fn load_rom<T>(&mut self, rom_get: &T)
    where
        for<'a> &'a T: Into<RomFile>,
    {
        let rom_file = rom_get.into();
        self.cpu.load_rom(&rom_file);
        self.ppu.borrow_mut().load_rom(&rom_file);
        self.rom_file = Some(rom_file);
    }

    pub fn save_state(&self) -> Option<SaveState> {
        let ppu_state = {
            let ppu_ref = self.ppu.borrow();
            PpuState::from(ppu_ref.deref())
        };

        self.rom_file.as_ref().map(|rom| SaveState {
            cpu: CpuState::from(&self.cpu),
            ppu: ppu_state,
            total_cycles: self.total_cycles,
            rom_file: rom.clone(),
            version: 1,
            ppu_cycle_counter: self.ppu_cycle_counter,
            cpu_cycle_counter: self.cpu_cycle_counter,
        })
    }

    pub fn load_state(&mut self, state: SaveState) {
        // Use the already loaded ROM file if available (it has the actual ROM data),
        // otherwise fall back to the savestate's ROM (which may have empty data due to Skip)
        let rom_to_use = self.rom_file.as_ref().unwrap_or(&state.rom_file);

        self.ppu = Rc::new(RefCell::new(Ppu::from(&state.ppu, rom_to_use)));

        self.cpu = Cpu::from(&state.cpu, self.ppu.clone(), rom_to_use);

        // Add PPU registers to CPU memory map (same as power() does)
        // This is critical - without this, the CPU can't communicate with the PPU!
        self.cpu.memory.add_memory(
            0x2000..=0x3FFF,
            Memory::MirrorMemory(MirrorMemory::new(
                Box::new(Memory::PpuRegisters(PpuRegisters::new(self.ppu.clone()))),
                0x0007,
            )),
        );

        // Only update rom_file if we didn't have one loaded
        if self.rom_file.is_none() {
            self.rom_file = Some(state.rom_file);
        }

        self.cpu_cycle_counter = state.cpu_cycle_counter;
        self.ppu_cycle_counter = state.ppu_cycle_counter;
    }

    #[inline(always)]
    pub fn step(&mut self) -> Result<ExecutionFinishedType, String> {
        self.step_internal(u128::MAX, false)
    }

    #[inline]
    fn step_internal(
        &mut self,
        last_cycle: u128,
        stop_at_frame: bool,
    ) -> Result<ExecutionFinishedType, String> {
        self.total_cycles += 1;
        self.cpu_cycle_counter = self.cpu_cycle_counter.wrapping_add(1);
        self.ppu_cycle_counter = self.ppu_cycle_counter.wrapping_add(1);

        // Only borrow PPU when vbl_clear_scheduled might be active
        // This check is only relevant immediately after reading PPU status
        {
            let ppu = self.ppu.borrow();
            if ppu.vbl_clear_scheduled.get().is_some() {
                ppu.vbl_reset_counter.set(ppu.vbl_reset_counter.get() + 1);
                ppu.process_vbl_clear_scheduled();
            }
        }

        if self.total_cycles > last_cycle {
            self.total_cycles -= 1;
            return Ok(ExecutionFinishedType::ReachedLastCycle);
        };

        let mut res = Ok(ExecutionFinishedType::CycleCompleted);
        let mut is_frame_done = false;

        if self.ppu_cycle_counter == 4 {
            is_frame_done = self.ppu.borrow_mut().step();
            self.ppu_cycle_counter = 0;
        }

        // Check if CPU should step (every 12th master cycle, offset by 2)
        // cpu_cycle_counter + 2 == 12  means cpu_cycle_counter == 10
        if self.cpu_cycle_counter == 10 {
            // Only check trace_log when actually needed
            let do_trace = self.trace_log.is_some()
                && matches!(&self.cpu.current_op, &MicroOp::FetchOpcode(..));

            res = self.cpu.step();

            if do_trace && let Some(ref mut trace) = self.trace_log {
                let ppu_state = {
                    let ppu_ref = self.ppu.borrow();
                    PpuState::from(ppu_ref.deref())
                };

                let state = SaveState {
                    cpu: CpuState::from(&self.cpu),
                    ppu: ppu_state,
                    ppu_cycle_counter: self.ppu_cycle_counter,
                    cpu_cycle_counter: self.cpu_cycle_counter,
                    total_cycles: self.total_cycles,
                    rom_file: self.rom_file.as_ref().unwrap().clone(),
                    version: 1,
                };

                trace.trace(state)
            }
        }

        if self.cpu_cycle_counter == 12 {
            self.cpu_cycle_counter = 0;
        }

        if stop_at_frame && is_frame_done {
            res = Ok(ExecutionFinishedType::FrameDone);
        }

        res
    }

    pub fn enable_trace(&mut self) { self.trace_log = Some(TraceLog::default()) }
}

impl Default for Nes {
    fn default() -> Self {
        let cpu = Cpu::new();
        let ppu = Rc::new(RefCell::new(Ppu::default()));
        Nes::new(cpu, ppu)
    }
}

#[derive(Debug)]
pub enum ExecutionFinishedType {
    ReachedLastCycle,
    ReachedHlt,
    CycleCompleted,
    FrameDone,
}
