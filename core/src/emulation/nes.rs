use std::cell::RefCell;
use std::fmt::Debug;
use std::ops::{Deref, RangeInclusive};
use std::rc::Rc;
use std::time::Duration;

use crate::emulation::cpu::{Cpu, MicroOp};
use crate::emulation::mem::Memory;
use crate::emulation::mem::mirror_memory::MirrorMemory;
use crate::emulation::mem::ppu_registers::PpuRegisters;
use crate::emulation::ppu::{EmulatorFetchable, Ppu};
use crate::emulation::rom::RomFile;
use crate::emulation::savestate::{CpuState, PpuState, SaveState};
use crate::trace::TraceLog;

/// Number of CPU cycles executed in a single NTSC frame (~29,780).
pub const CPU_CYCLES_PER_FRAME: u16 = 29780;

/// Duration of a single NTSC frame (~16.67 ms, targeting ~60 FPS).
pub const FRAME_DURATION: Duration = Duration::from_nanos(16_666_667);

/// Number of master clock cycles per NTSC frame (357,366).
///
/// The NES master clock runs at ~21.477 MHz. The CPU divides this by 12 and
/// the PPU divides it by 4, so one master cycle is the finest timing granularity.
pub const MASTER_CYCLES_PER_FRAME: u32 = 357366;

/// The top-level NES emulator.
///
/// `Nes` orchestrates the CPU, PPU, and memory subsystems to provide
/// cycle-accurate emulation of the Nintendo Entertainment System. It is the
/// primary entry point for consumers of this library.
///
/// # Lifecycle
///
/// 1. Create an instance with [`Nes::default()`] or [`Nes::new()`].
/// 2. Load a ROM with [`load_rom()`](Nes::load_rom).
/// 3. Power on with [`power()`](Nes::power).
/// 4. Advance emulation with [`step_frame()`](Nes::step_frame), [`step()`](Nes::step),
///    or [`run()`](Nes::run).
/// 5. Read the rendered output with [`get_pixel_buffer()`](Nes::get_pixel_buffer).
///
/// # Example
///
/// ```rust,no_run
/// use monsoon_core::emulation::nes::Nes;
/// use monsoon_core::emulation::rom::RomFile;
///
/// let mut nes = Nes::default();
/// let rom_bytes = std::fs::read("game.nes").unwrap();
/// let rom = RomFile::load(&rom_bytes, None).unwrap();
/// nes.load_rom(&rom);
/// nes.power();
///
/// // Run one frame
/// nes.step_frame().expect("emulation error");
///
/// // Read the pixel buffer (palette indices, not RGB)
/// let pixels = nes.get_pixel_buffer();
/// ```
pub struct Nes {
    /// The MOS 6502 CPU instance.
    pub(crate) cpu: Cpu,
    /// The 2C02 PPU instance, wrapped in `Rc<RefCell<_>>` because the CPU
    /// and PPU share access (e.g., through memory-mapped PPU registers).
    pub(crate) ppu: Rc<RefCell<Ppu>>,
    /// Total master clock cycles elapsed since power-on.
    pub total_cycles: u128,
    /// The currently loaded ROM, or `None` if no ROM has been loaded.
    pub rom_file: Option<RomFile>,
    /// Optional CPU instruction trace logger for debugging.
    pub(crate) trace_log: Option<TraceLog>,
    /// Internal CPU clock divider counter (0-12).
    pub(crate) cpu_cycle_counter: u8,
    /// Internal PPU clock divider counter (0-4).
    pub(crate) ppu_cycle_counter: u8,
}

impl Nes {
    /// Returns the current pixel buffer as a vector of 16-bit palette indices.
    ///
    /// Each value encodes:
    /// - **Bits 0-5**: NES color index (0-63 from the system palette).
    /// - **Bits 6-8**: Emphasis bits (R, G, B emphasis from the PPU mask register).
    ///
    /// The buffer has dimensions [`TOTAL_OUTPUT_WIDTH`](crate::emulation::ppu::TOTAL_OUTPUT_WIDTH)
    /// × [`TOTAL_OUTPUT_HEIGHT`](crate::emulation::ppu::TOTAL_OUTPUT_HEIGHT) (256 × 240) pixels,
    /// stored in row-major order.
    ///
    /// To convert these indices to RGB colors, use a
    /// [`ScreenRenderer`](crate::emulation::screen_renderer::ScreenRenderer) implementation.
    #[inline]
    pub fn get_pixel_buffer(&self) -> Vec<u16> { self.ppu.borrow().pixel_buffer.clone() }

    /// Powers on the emulator, initializing the CPU-PPU connection.
    ///
    /// This sets up memory-mapped PPU registers in the CPU address space
    /// (addresses `$2000`-`$3FFF`) and performs the CPU reset sequence.
    /// Must be called after [`load_rom()`](Nes::load_rom) and before any
    /// execution methods.
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

    /// Powers off the emulator, resetting all state to defaults.
    ///
    /// After calling this, you must call [`load_rom()`](Nes::load_rom) and
    /// [`power()`](Nes::power) again before resuming emulation.
    pub fn power_off(&mut self) {
        self.cpu = Cpu::default();
        self.ppu = Rc::new(RefCell::new(Ppu::default()));
        self.total_cycles = 0;
        self.cpu_cycle_counter = 0;
        self.ppu_cycle_counter = 0;
    }

    /// Runs the emulator indefinitely until a halt instruction (`HLT`) is encountered.
    ///
    /// # Returns
    ///
    /// - `Ok(ExecutionFinishedType::ReachedHlt)` when the CPU executes a halt instruction.
    ///
    /// # Panics
    ///
    /// Panics if an internal emulation error occurs during execution.
    pub fn run(&mut self) -> Result<ExecutionFinished, String> {
        self.run_until(u128::MAX, RunOptions::default())
    }

    /// Runs the emulator until a specific cycle count is reached, or until a frame
    /// boundary if `stop_at_frame` is `true`.
    ///
    /// # Arguments
    ///
    /// * `last_cycle` — The master clock cycle count at which to stop execution.
    ///   Use `u128::MAX` to run without a cycle limit.
    /// * `stop_at_frame` — If `true`, execution also stops at the end of the
    ///   current video frame (after scanline 240).
    ///
    /// # Returns
    ///
    /// - `Ok(ExecutionFinishedType::ReachedLastCycle)` when `last_cycle` is exceeded.
    /// - `Ok(ExecutionFinishedType::FrameDone)` when a frame boundary is reached
    ///   (only if `stop_at_frame` is `true`).
    /// - `Ok(ExecutionFinishedType::ReachedHlt)` when the CPU halts.
    ///
    /// # Panics
    ///
    /// Panics if an internal emulation error occurs during execution.
    pub fn run_until(
        &mut self,
        last_cycle: u128,
        run_option: RunOptions,
    ) -> Result<ExecutionFinished, String> {
        loop {
            let res = self.step_internal(last_cycle);
            match res {
                Ok(res) => {
                    if run_option.stop_at_scanline && res.scanline_done {
                        return Ok(res);
                    }

                    if run_option.stop_at_frame && res.frame_done {
                        return Ok(res);
                    }

                    if run_option.stop_at_cpu_cycle && res.cpu_cycle_completed {
                        return Ok(res);
                    }

                    if run_option.stop_at_ppu_cycle && res.ppu_cycle_completed {
                        return Ok(res);
                    }

                    if res.last_cycle_reached || res.hlt_reached {
                        return Ok(res);
                    }

                    if res.cycle_completed {
                        continue;
                    }

                    return Ok(res);
                }
                Err(err) => {
                    panic!("{}", err)
                }
            };
        }
    }

    /// Returns a debug snapshot of memory contents.
    ///
    /// Returns a vector containing two memory dumps:
    /// - Index 0: CPU memory (the full 64 KB address space, or the requested range).
    /// - Index 1: PPU memory (VRAM, or the requested range).
    ///
    /// # Arguments
    ///
    /// * `range` — An optional address range to read. If `None`, dumps all mapped memory.
    ///
    /// This is a side-effect-free read intended for debugger UIs.
    pub fn get_memory_debug(&self, range: Option<RangeInclusive<u16>>) -> Vec<Vec<u8>> {
        vec![
            self.cpu.get_memory_debug(range.clone()),
            self.ppu.borrow().get_memory_debug(range.clone()),
        ]
    }

    /// Runs the emulator for exactly one video frame (until the PPU completes
    /// scanline 240 and enters vertical blanking).
    ///
    /// This is the recommended method for frame-based emulation loops.
    #[inline]
    pub fn step_frame(&mut self) -> Result<ExecutionFinished, String> {
        self.run_until(
            u128::MAX,
            RunOptions {
                stop_at_frame: true,
                ..Default::default()
            },
        )
    }

    /// Runs the emulator until the next scanline completion (until the PPU completes
    /// dot 340).
    #[inline]
    pub fn step_scanline(&mut self) -> Result<ExecutionFinished, String> {
        self.run_until(
            u128::MAX,
            RunOptions {
                stop_at_scanline: true,
                ..Default::default()
            },
        )
    }

    /// Runs the emulator for until the next cpu cycle completes (<=12 master cycles)
    #[inline]
    pub fn step_cpu_cycle(&mut self) -> Result<ExecutionFinished, String> {
        self.run_until(
            u128::MAX,
            RunOptions {
                stop_at_cpu_cycle: true,
                ..Default::default()
            },
        )
    }

    /// Runs the emulator for until the next ppu cycle completes (<=4 master cycles)
    #[inline]
    pub fn step_ppu_cycle(&mut self) -> Result<ExecutionFinished, String> {
        self.run_until(
            u128::MAX,
            RunOptions {
                stop_at_ppu_cycle: true,
                ..Default::default()
            },
        )
    }
}

impl Nes {
    /// Creates a new `Nes` instance with the given CPU and PPU.
    ///
    /// For most use cases, prefer [`Nes::default()`] which creates a standard
    /// NES configuration. Use this constructor when you need to supply a
    /// pre-configured CPU or PPU (e.g., for testing).
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

    /// Performs a soft reset of the CPU and PPU.
    ///
    /// This is equivalent to pressing the reset button on the NES console.
    /// The ROM remains loaded, but the CPU program counter is reloaded from
    /// the reset vector (`$FFFC`).
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.ppu.borrow_mut().reset();
    }

    /// Loads a ROM into the emulator.
    ///
    /// The argument can be anything that converts to a [`RomFile`] by reference.
    /// Common conversions include:
    ///
    /// - `&RomFile` — a pre-parsed ROM file.
    /// - `&[u8]` — raw ROM bytes (name is set to `None`).
    /// - `&(&[u8], String)` — raw ROM bytes with a name.
    /// - `&String` — a file path (native only, not available on WASM).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use monsoon_core::emulation::nes::Nes;
    /// use monsoon_core::emulation::rom::RomFile;
    ///
    /// let mut nes = Nes::default();
    /// let data = std::fs::read("game.nes").unwrap();
    /// let rom = RomFile::load(&data, None).unwrap();
    /// nes.load_rom(&rom);
    /// ```
    pub fn load_rom<T>(&mut self, rom_get: &T)
    where
        for<'a> &'a T: Into<RomFile>,
    {
        let rom_file = rom_get.into();
        self.cpu.load_rom(&rom_file);
        self.ppu.borrow_mut().load_rom(&rom_file);
        self.rom_file = Some(rom_file);
    }

    /// Captures the current emulator state as a [`SaveState`].
    ///
    /// Returns `None` if no ROM is currently loaded. The returned state can
    /// be serialized with [`ToBytes::to_bytes()`](crate::util::ToBytes::to_bytes)
    /// and later restored with [`load_state()`](Nes::load_state).
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

    /// Restores the emulator to a previously captured [`SaveState`].
    ///
    /// If a ROM is currently loaded, its data is preserved (the save state's ROM
    /// metadata is used only when no ROM is loaded). After loading, execution can
    /// resume from the exact point where the state was saved.
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

    /// Advances the emulator by exactly one master clock cycle.
    ///
    /// This is the finest-grained execution method. During each master cycle,
    /// either the CPU, the PPU, or both may perform work depending on their
    /// respective clock dividers.
    ///
    /// For most use cases, prefer [`step_frame()`](Nes::step_frame).
    #[inline]
    pub fn step(&mut self) -> Result<ExecutionFinished, String> { self.step_internal(u128::MAX) }

    #[inline(always)]
    fn step_internal(&mut self, last_cycle: u128) -> Result<ExecutionFinished, String> {
        let mut res = ExecutionFinished {
            cycle_completed: true,
            ..Default::default()
        };

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
            res.last_cycle_reached = true;
            return Ok(res);
        };

        if self.ppu_cycle_counter == 4 {
            res = res.merge(self.ppu.borrow_mut().step());
            res.ppu_cycle_completed = true;
            self.ppu_cycle_counter = 0;
        }

        // Check if CPU should step (every 12th master cycle, offset by 2)
        // cpu_cycle_counter + 2 == 12  means cpu_cycle_counter == 10
        if self.cpu_cycle_counter == 10 {
            // Only check trace_log when actually needed
            let do_trace =
                self.trace_log.is_some() && matches!(&self.cpu.current_op, &MicroOp::FetchOpcode);

            let cpu_res = self.cpu.step();

            #[allow(clippy::question_mark)]
            if let Ok(cpu_res) = cpu_res {
                res = res.merge(cpu_res);
                res.cpu_cycle_completed = true;
            } else {
                return cpu_res;
            }

            if do_trace {
                self.write_trace_log();
            }
        }

        if self.cpu_cycle_counter == 12 {
            self.cpu_cycle_counter = 0;
        }

        Ok(res)
    }

    /// Enables CPU instruction tracing for debugging purposes.
    ///
    /// When enabled, every executed instruction is logged to an internal
    /// trace buffer in a nestest-compatible format. This is primarily useful
    /// for verifying CPU accuracy against reference logs.
    pub fn enable_trace(&mut self) { self.trace_log = Some(TraceLog::default()) }

    /// Cold path: Write a trace log entry (only called when tracing is enabled)
    #[cold]
    #[inline(never)]
    fn write_trace_log(&mut self) {
        if let Some(ref mut trace) = self.trace_log {
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
}

impl Default for Nes {
    fn default() -> Self {
        let cpu = Cpu::new();
        let ppu = Rc::new(RefCell::new(Ppu::default()));
        Nes::new(cpu, ppu)
    }
}

impl Nes {
    // --- CPU debug accessors ---

    /// Returns the current program counter value.
    pub fn program_counter(&self) -> u16 { self.cpu.program_counter }

    /// Returns the opcode byte of the instruction currently being executed.
    pub fn current_opcode_byte(&self) -> Option<u8> { self.cpu.current_opcode.map(|c| c.opcode) }

    /// Returns `true` if the CPU has executed a halt (KIL) instruction.
    pub fn is_halted(&self) -> bool { self.cpu.is_halted }

    /// Returns the last memory access (address, was_read), or `None` if no access occurred.
    pub fn last_memory_access(&self) -> Option<(u16, bool, u8)> { self.cpu.last_memory_access }

    // --- PPU debug accessors ---

    /// Returns `true` if the current frame is an even frame.
    pub fn is_even_frame(&self) -> bool { self.ppu.borrow().even_frame }

    /// Returns `true` if the PPU is currently rendering (background or sprites enabled).
    pub fn is_rendering(&self) -> bool { self.ppu.borrow().is_rendering() }

    /// Returns debug palette data from the PPU.
    pub fn get_palettes_debug(&self) -> EmulatorFetchable { self.ppu.borrow().get_palettes_debug() }

    /// Returns debug tile data from the PPU.
    pub fn get_tiles_debug(&self) -> EmulatorFetchable { self.ppu.borrow().get_tiles_debug() }

    /// Returns debug nametable data from the PPU.
    pub fn get_nametable_debug(&self) -> EmulatorFetchable {
        self.ppu.borrow().get_nametable_debug()
    }

    pub fn get_sprites_debug(&self) -> EmulatorFetchable { self.ppu.borrow().get_sprites_debug() }

    pub fn get_soam_sprites_debug(&self) -> EmulatorFetchable {
        self.ppu.borrow().get_soam_sprites_debug()
    }

    /// Returns OAM (sprite memory) contents for debugging.
    pub fn get_oam_debug(&self) -> Vec<u8> { self.ppu.borrow().oam.get_memory_debug(None) }

    // --- Memory write methods ---

    /// Writes a value to CPU memory at the given address (for initialization/debugging).
    pub fn cpu_mem_write(&mut self, addr: u16, value: u8) {
        self.cpu.memory.mem_write(addr, value);
    }

    /// Initializes CPU memory at the given address (for initialization/debugging).
    pub fn cpu_mem_init(&mut self, addr: u16, data: u8) { self.cpu.memory.init(addr, data); }

    /// Writes a value to PPU memory at the given address (for initialization/debugging).
    pub fn ppu_mem_write(&self, addr: u16, value: u8) {
        self.ppu.borrow_mut().memory.mem_write(addr, value);
    }

    /// Initializes PPU memory at the given address (for initialization/debugging).
    pub fn ppu_mem_init(&self, addr: u16, data: u8) { self.ppu.borrow_mut().mem_init(addr, data); }

    /// Writes a value to OAM (sprite memory) at the given address (for initialization/debugging).
    pub fn oam_write(&self, addr: u16, value: u8) {
        self.ppu.borrow_mut().oam.mem_write(addr, value);
    }

    /// Returns a reference to the trace log, if tracing is enabled.
    pub fn trace_log(&self) -> Option<&TraceLog> { self.trace_log.as_ref() }
}

/// Describes why an emulation run finished.
///
/// Returned by [`Nes::run()`], [`Nes::run_until()`], [`Nes::step()`], and
/// [`Nes::step_frame()`] to indicate the reason execution stopped.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct ExecutionFinished {
    pub last_cycle_reached: bool,
    pub hlt_reached: bool,
    pub cycle_completed: bool,
    pub cpu_cycle_completed: bool,
    pub ppu_cycle_completed: bool,
    pub frame_done: bool,
    pub scanline_done: bool,
}

impl ExecutionFinished {
    pub fn merge(self, with: ExecutionFinished) -> Self {
        Self {
            last_cycle_reached: self.last_cycle_reached || with.last_cycle_reached,
            hlt_reached: self.hlt_reached || with.hlt_reached,
            cycle_completed: self.cycle_completed || with.cycle_completed,
            cpu_cycle_completed: self.cpu_cycle_completed || with.cpu_cycle_completed,
            ppu_cycle_completed: self.ppu_cycle_completed || with.ppu_cycle_completed,
            frame_done: self.frame_done || with.frame_done,
            scanline_done: self.scanline_done || with.scanline_done,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct RunOptions {
    pub stop_at_frame: bool,
    pub stop_at_scanline: bool,
    pub stop_at_ppu_cycle: bool,
    pub stop_at_cpu_cycle: bool,
}
