//! Execution engine for CLI-driven emulation.
//!
//! This module provides a generic, extensible execution engine that can run
//! emulation until various stop conditions are met. It's designed to be usable
//! both from the CLI and as a Rust crate API.
//!
//! # Design Goals
//! - Generic stop condition system that's easy to extend
//! - Support for frames, cycles, PC breakpoints, memory conditions
//! - Clean separation from CLI argument parsing
//! - Suitable for exposing as a crate API

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use monsoon_core::emulation::nes::{MASTER_CYCLES_PER_FRAME, Nes, RunOptions};
use monsoon_core::emulation::savestate::{SaveState, try_load_state_from_bytes};
use monsoon_core::util::ToBytes;

use crate::cli::args::parse_hex_u8;
// =============================================================================
// Stop Conditions
// =============================================================================

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

/// Reason why execution stopped
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Reached target cycle count
    CyclesReached(u128),
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
    /// No stop condition was set (ran to completion or max cycles)
    Completed,
}

/// A stop condition that can be checked during execution
#[derive(Debug, Clone)]
pub enum StopCondition {
    /// Stop after N master cycles
    Cycles(u128),
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

    pub fn check(&self, emu: &mut Nes, cycles: u128, frames: u64) -> bool {
        match self {
            StopCondition::Cycles(target) => cycles >= *target,
            StopCondition::Frames(target) => frames >= *target,
            StopCondition::PcEquals(addr) | StopCondition::Breakpoint(addr) => {
                emu.program_counter() == *addr
            }
            StopCondition::Opcode(op) => {
                if let Some(opcode) = emu.current_opcode_byte()
                    && opcode == *op
                {
                    return true;
                }

                false
            }
            StopCondition::MemoryEquals {
                addr,
                value,
                and,
            } => {
                let and = and.as_ref().map(|and| and.check(emu, cycles, frames));

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
                let and = and.as_ref().map(|and| and.check(emu, cycles, frames));

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

    pub fn reason(&self, emu: &mut Nes, cycles: u128, frames: u64) -> StopReason {
        match self {
            StopCondition::Cycles(_) => StopReason::CyclesReached(cycles),
            StopCondition::Frames(_) => StopReason::FramesReached(frames),
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

// =============================================================================
// Execution Configuration
// =============================================================================

/// Configuration for an execution run.
///
/// This struct is designed to be constructed either from CLI arguments
/// or programmatically when using the crate as a library.
#[derive(Debug, Clone, Default)]
pub struct ExecutionConfig {
    /// Stop conditions (first one met will stop execution)
    pub stop_conditions: Vec<StopCondition>,
    /// Whether to stop on any HLT instruction
    pub stop_on_halt: bool,
    /// Path to trace log file (if any)
    pub trace_path: Option<PathBuf>,
    /// Verbose output
    pub verbose: bool,
}

impl ExecutionConfig {
    /// Create a new empty execution config
    pub fn new() -> Self { Self::default() }

    /// Add a stop condition
    pub fn with_stop_condition(mut self, condition: StopCondition) -> Self {
        self.stop_conditions.push(condition);
        self
    }

    /// Set stop after N cycles
    pub fn with_cycles(mut self, cycles: u128) -> Self {
        self.stop_conditions.push(StopCondition::Cycles(cycles));
        self
    }

    /// Set stop after N frames
    pub fn with_frames(mut self, frames: u64) -> Self {
        self.stop_conditions.push(StopCondition::Frames(frames));
        self
    }

    /// Set stop when PC equals address
    pub fn with_pc_breakpoint(mut self, addr: u16) -> Self {
        self.stop_conditions.push(StopCondition::PcEquals(addr));
        self
    }

    /// Add a breakpoint (alias for with_pc_breakpoint)
    pub fn with_breakpoint(mut self, addr: u16) -> Self {
        self.stop_conditions.push(StopCondition::PcEquals(addr));
        self
    }

    /// Add a memory watchpoint
    pub fn with_memory_watch(mut self, addr: u16, access_type: MemoryAccessType) -> Self {
        self.stop_conditions.push(StopCondition::MemoryWatch {
            addr,
            access_type,
        });
        self
    }

    /// Set trace log path
    pub fn with_trace(mut self, path: PathBuf) -> Self {
        self.trace_path = Some(path);
        self
    }

    /// Enable verbose output
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Enable stop on HLT
    pub fn with_stop_on_halt(mut self, stop: bool) -> Self {
        self.stop_on_halt = stop;
        self
    }

    /// Calculate the maximum cycles to run based on stop conditions
    fn max_cycles(&self) -> u128 {
        let mut max = u128::MAX;
        for cond in &self.stop_conditions {
            match cond {
                StopCondition::Cycles(c) => max = max.min(*c),
                StopCondition::Frames(f) => {
                    max = max.min(*f as u128 * MASTER_CYCLES_PER_FRAME as u128)
                }
                _ => {}
            }
        }
        max
    }

    /// Check if any stop condition is met
    fn check_conditions(&self, emu: &mut Nes, cycles: u128, frames: u64) -> Option<StopReason> {
        for cond in &self.stop_conditions {
            if cond.check(emu, cycles, frames) {
                return Some(cond.reason(emu, cycles, frames));
            }
        }

        None
    }
}

// =============================================================================
// Execution Result
// =============================================================================

/// Result of an execution run
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Why execution stopped
    pub stop_reason: StopReason,
    /// Total cycles executed
    pub total_cycles: u128,
    /// Total frames executed
    pub total_frames: u64,
}

// =============================================================================
// Savestate Configuration
// =============================================================================

/// Source for loading a savestate
#[derive(Debug, Clone)]
pub enum SavestateSource {
    /// Load from file
    File(PathBuf),
    /// Load from stdin
    Stdin,
    /// Load from bytes (for programmatic use)
    Bytes(Vec<u8>),
}

/// Destination for saving a savestate
#[derive(Debug, Clone)]
pub enum SavestateDestination {
    /// Save to file
    File(PathBuf),
    /// Save to stdout
    Stdout,
}

// Re-export SavestateFormat from args for use in this module
pub use crate::cli::args::SavestateFormat;
use crate::cli::{CliArgs, parse_hex_u16};

/// Configuration for savestate operations
#[derive(Debug, Clone, Default)]
pub struct SavestateConfig {
    /// Source to load savestate from (if any)
    pub load_from: Option<SavestateSource>,
    /// Destination to save savestate to (if any)
    pub save_to: Option<SavestateDestination>,
    /// Format for saving savestates
    pub format: SavestateFormat,
}

impl SavestateConfig {
    /// Create a new empty savestate config
    pub fn new() -> Self { Self::default() }

    /// Set load source to file
    pub fn load_from_file(mut self, path: PathBuf) -> Self {
        self.load_from = Some(SavestateSource::File(path));
        self
    }

    /// Set load source to stdin
    pub fn load_from_stdin(mut self) -> Self {
        self.load_from = Some(SavestateSource::Stdin);
        self
    }

    /// Set save destination to file
    pub fn save_to_file(mut self, path: PathBuf) -> Self {
        self.save_to = Some(SavestateDestination::File(path));
        self
    }

    /// Set save destination to stdout
    pub fn save_to_stdout(mut self) -> Self {
        self.save_to = Some(SavestateDestination::Stdout);
        self
    }

    /// Set savestate format
    pub fn with_format(mut self, format: SavestateFormat) -> Self {
        self.format = format;
        self
    }
}

// =============================================================================
// Execution Engine
// =============================================================================

/// The main execution engine for CLI-driven emulation.
///
/// This struct manages the emulator lifecycle and provides a clean API
/// for running emulation with various configurations.
///
/// # Video Export Modes
///
/// - **Buffered mode** (default): All frames are stored in memory, then encoded
///   at the end. Suitable for small exports or when you need access to all
///   frames.
///
/// - **Streaming mode**: Frames are encoded immediately as they are generated.
///   Use `run_with_video_encoder()` for this mode. Significantly reduces memory
///   usage for long recordings.
pub struct ExecutionEngine {
    /// The emulator instance
    pub emu: Nes,
    /// Execution configuration
    pub config: ExecutionConfig,
    /// Savestate configuration
    pub savestate_config: SavestateConfig,
    /// Collected frames (used in buffered mode) - raw palette indices
    pub frames: Vec<Vec<u16>>,
    /// Track current frame count
    frame_count: u64,
    /// Whether to collect frames (set to false for streaming mode)
    collect_frames: bool,
}

impl ExecutionEngine {
    /// Create a new execution engine with default emulator
    pub fn new() -> Self {
        Self {
            emu: Nes::default(),
            config: ExecutionConfig::new(),
            savestate_config: SavestateConfig::new(),
            frames: vec![],
            frame_count: 0,
            collect_frames: true,
        }
    }

    /// Create execution engine with existing emulator
    pub fn with_emulator(emu: Nes) -> Self {
        Self {
            emu,
            config: ExecutionConfig::new(),
            savestate_config: SavestateConfig::new(),
            frames: vec![],
            frame_count: 0,
            collect_frames: true,
        }
    }

    /// Set execution configuration
    pub fn with_config(mut self, config: ExecutionConfig) -> Self {
        self.config = config;
        self
    }

    /// Set savestate configuration
    pub fn with_savestate_config(mut self, config: SavestateConfig) -> Self {
        self.savestate_config = config;
        self
    }

    /// Load ROM from path
    pub fn load_rom(&mut self, path: &Path) -> Result<(), String> {
        let path_str = path.to_string_lossy().to_string();
        self.emu.load_rom(&path_str);
        Ok(())
    }

    /// Power on the emulator
    pub fn power_on(&mut self) { self.emu.power(); }

    /// Power off the emulator
    pub fn power_off(&mut self) { self.emu.power_off(); }

    /// Reset the emulator
    pub fn reset(&mut self) { self.emu.reset(); }

    /// Load savestate based on configuration
    pub fn load_savestate(&mut self) -> Result<(), String> {
        if let Some(ref source) = self.savestate_config.load_from {
            let state = match source {
                SavestateSource::File(path) => {
                    let data = std::fs::read(path).map_err(|e| {
                        format!("Failed to read savestate from {}: {}", path.display(), e)
                    })?;
                    try_load_state_from_bytes(&data).ok_or_else(|| {
                        format!("Failed to load savestate from {}", path.display())
                    })?
                }
                SavestateSource::Stdin => {
                    let mut buffer = Vec::new();
                    std::io::stdin()
                        .read_to_end(&mut buffer)
                        .map_err(|e| format!("Failed to read savestate from stdin: {}", e))?;
                    decode_savestate(&buffer)?
                }
                SavestateSource::Bytes(bytes) => decode_savestate(bytes)?,
            };
            self.emu.load_state(state);
        }
        Ok(())
    }

    /// Save savestate based on configuration
    pub fn save_savestate(&self) -> Result<(), String> {
        if let Some(ref dest) = self.savestate_config.save_to {
            let state = self
                .emu
                .save_state()
                .ok_or_else(|| "No ROM loaded, cannot save state".to_string())?;
            let encoded = encode_savestate(&state, self.savestate_config.format)?;

            match dest {
                SavestateDestination::File(path) => {
                    std::fs::write(path, &encoded).map_err(|e| {
                        format!("Failed to write savestate to {}: {}", path.display(), e)
                    })?;
                }
                SavestateDestination::Stdout => {
                    std::io::stdout()
                        .write_all(&encoded)
                        .map_err(|e| format!("Failed to write savestate to stdout: {}", e))?;
                }
            }
        }
        Ok(())
    }

    /// Run execution until a stop condition is met
    pub fn run(&mut self) -> Result<ExecutionResult, String> {
        // Set up trace if configured
        if self.config.trace_path.is_some() {
            self.emu.enable_trace();
        }

        let max_cycles = self.config.max_cycles();
        let start_cycles = self.emu.total_cycles;

        // Run frame by frame for stop condition checking
        let result = loop {
            // Run one frame
            match self.emu.step_frame() {
                Ok(_) => {}
                Err(e) => {
                    break ExecutionResult {
                        stop_reason: StopReason::Error(e),
                        total_cycles: self.emu.total_cycles - start_cycles,
                        total_frames: self.frame_count,
                    };
                }
            }

            // Only collect frames if in buffered mode
            if self.collect_frames {
                self.frames.push(self.emu.get_pixel_buffer());
            }

            self.frame_count += 1;
            let cycles_run = self.emu.total_cycles - start_cycles;

            // Check stop conditions
            if let Some(reason) =
                self.config
                    .check_conditions(&mut self.emu, cycles_run, self.frame_count)
            {
                break ExecutionResult {
                    stop_reason: reason,
                    total_cycles: cycles_run,
                    total_frames: self.frame_count,
                };
            }

            // Check max cycles
            if self.emu.total_cycles >= max_cycles {
                break ExecutionResult {
                    stop_reason: StopReason::Completed,
                    total_cycles: cycles_run,
                    total_frames: self.frame_count,
                };
            }
        };

        // Write trace log to file if configured
        self.write_trace_log()?;

        Ok(result)
    }

    /// Run execution with streaming video export.
    ///
    /// This mode writes frames directly to the video encoder as they are
    /// generated, instead of buffering all frames in memory. This
    /// significantly reduces memory usage for long recordings.
    ///
    /// # Arguments
    ///
    /// * `encoder` - A streaming video encoder that will receive frames as
    ///   they're generated
    ///
    /// # Performance
    ///
    /// - Uses parallel upscaling via rayon (if encoder has upscaling enabled)
    /// - O(1) memory usage per frame instead of O(n) for all frames
    /// - Frames are written immediately, reducing peak memory usage
    ///
    /// # FPS Multipliers
    ///
    /// When the encoder's FPS config specifies a multiplier > 1 (e.g., 2x, 3x),
    /// this method captures frames at sub-frame intervals. For example:
    /// - 2x: Captures at mid-frame and end of frame (2 captures per PPU frame)
    /// - 3x: Captures at 1/3, 2/3, and end of frame (3 captures per PPU frame)
    ///
    /// This produces true intermediate states showing partial rendering
    /// progress.
    pub fn run_with_video_encoder(
        &mut self,
        encoder: &mut super::video::StreamingVideoEncoder,
        renderer: &mut Box<dyn monsoon_core::emulation::screen_renderer::ScreenRenderer>,
    ) -> Result<ExecutionResult, String> {
        // Disable frame collection for streaming mode
        self.collect_frames = false;

        // Set up trace if configured
        if self.config.trace_path.is_some() {
            self.emu.enable_trace();
        }

        let max_cycles = self.config.max_cycles();
        let start_cycles = self.emu.total_cycles;

        // Get the number of captures per PPU frame from the encoder's FPS config
        let captures_per_frame = encoder.captures_per_frame();

        // Run frame by frame for stop condition checking
        loop {
            // Track the start of this PPU frame to calculate capture targets
            // This avoids accumulated rounding errors from integer division
            let frame_start_cycles = self.emu.total_cycles;

            // Run partial frames based on FPS multiplier and capture at each interval
            for capture_idx in 0..captures_per_frame {
                // Calculate target cycle for this capture relative to frame start
                // Using (capture_idx + 1) * MASTER_CYCLES_PER_FRAME / captures_per_frame
                // ensures the final capture always aligns with the frame boundary
                let odd_frame_offset:i32 = if self.emu.is_even_frame() && self.emu.is_rendering() {
                    2
                } else {
                    -2
                };

                let base = (capture_idx + 1) as u128 * MASTER_CYCLES_PER_FRAME as u128;

                let base = if odd_frame_offset >= 0 {
                    base.saturating_add(odd_frame_offset as u128)
                } else {
                    base.saturating_sub((-odd_frame_offset) as u128)
                };

                let capture_point = base / captures_per_frame as u128;
                let target_cycles = frame_start_cycles + capture_point;

                // Run until the target cycle
                match self.emu.run_until(target_cycles, RunOptions::default()) {
                    Ok(_) => {}
                    Err(e) => {
                        return Ok(ExecutionResult {
                            stop_reason: StopReason::Error(e),
                            total_cycles: self.emu.total_cycles - start_cycles,
                            total_frames: self.frame_count,
                        });
                    }
                }

                // Write frame directly to encoder (with upscaling if configured)
                // This captures the current pixel buffer state, which may be mid-render
                let frame = self.emu.get_pixel_buffer();
                let rgb_frame = renderer.buffer_to_image(&frame);
                encoder
                    .write_frame(rgb_frame)
                    .map_err(|e| format!("Video encoding error: {}", e))?;

                // Only increment frame_count at the end of a full PPU frame
                // (when we've done all captures for this frame)
                if capture_idx == captures_per_frame - 1 {
                    self.frame_count += 1;
                }
            }

            let cycles_run = self.emu.total_cycles - start_cycles;

            // Check stop conditions
            if let Some(reason) =
                self.config
                    .check_conditions(&mut self.emu, cycles_run, self.frame_count)
            {
                self.write_trace_log()?;
                return Ok(ExecutionResult {
                    stop_reason: reason,
                    total_cycles: cycles_run,
                    total_frames: self.frame_count,
                });
            }

            // Check max cycles
            if self.emu.total_cycles >= max_cycles {
                self.write_trace_log()?;
                return Ok(ExecutionResult {
                    stop_reason: StopReason::Completed,
                    total_cycles: cycles_run,
                    total_frames: self.frame_count,
                });
            }
        }
    }

    /// Enable or disable frame collection.
    ///
    /// When disabled, frames are not stored in memory during execution.
    /// Use this for streaming mode or when you don't need frame data.
    pub fn set_collect_frames(&mut self, collect: bool) { self.collect_frames = collect; }

    /// Get reference to the emulator
    pub fn emulator(&mut self) -> &mut Nes { &mut self.emu }

    /// Get mutable reference to the emulator
    pub fn emulator_mut(&mut self) -> &mut Nes { &mut self.emu }

    /// Write trace log to the configured file path, if tracing was enabled.
    fn write_trace_log(&self) -> Result<(), String> {
        if let Some(ref path) = self.config.trace_path
            && let Some(trace) = self.emu.trace_log()
        {
            std::fs::write(path, &trace.log)
                .map_err(|e| format!("Failed to write trace log to {}: {}", path.display(), e))?;
        }
        Ok(())
    }
}

impl Default for ExecutionEngine {
    fn default() -> Self { Self::new() }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Decode a savestate from bytes (auto-detects format).
///
/// Detection strategy: Try JSON first, then binary as fallback.
/// This is more robust than checking for `{` which could fail with
/// whitespace-prefixed JSON or misidentify binary data.
fn decode_savestate(bytes: &[u8]) -> Result<SaveState, String> {
    try_load_state_from_bytes(bytes)
        .ok_or_else(|| "Failed to decode savestate (tried all supported formats)".to_string())
}

/// Encode a savestate to bytes in the specified format
fn encode_savestate(state: &SaveState, format: SavestateFormat) -> Result<Vec<u8>, String> {
    match format {
        SavestateFormat::Binary => Ok(state.to_bytes(None)),
        SavestateFormat::Json => Ok(state.to_bytes(Some("json".to_string()))),
    }
}

// =============================================================================
// Builder from CLI Args
// =============================================================================

impl ExecutionConfig {
    /// Build execution config from CLI arguments
    pub fn from_cli_args(args: &CliArgs) -> Self {
        let mut config = Self::new();

        // Add cycle/frame stop conditions
        if let Some(cycles) = args.execution.cycles {
            config.stop_conditions.push(StopCondition::Cycles(cycles));
        }
        if let Some(frames) = args.execution.frames {
            config.stop_conditions.push(StopCondition::Frames(frames));
        }

        // Add opcode stop condition
        if let Some(op) = args.execution.until_opcode {
            config.stop_conditions.push(StopCondition::Opcode(op));
        }

        // Add memory condition
        if let Some(ref mem_cond) = args.execution.until_mem
            && let Ok(cond) = StopCondition::parse_memory_condition(mem_cond)
        {
            config.stop_conditions.extend(cond);
        }

        // Add memory watchpoints
        if !args.execution.watch_mem.is_empty()
            && let Ok(watches) = StopCondition::parse_memory_watches(&args.execution.watch_mem)
        {
            config.stop_conditions.extend(watches);
        }

        // Add HLT stop
        if args.execution.until_hlt {
            config.stop_on_halt = true;
        }

        // Add breakpoints (these are now the only way to stop at a PC address)
        for bp in &args.execution.breakpoint {
            config.stop_conditions.push(StopCondition::PcEquals(*bp));
        }

        // Add trace
        config.trace_path = args.execution.trace.clone();

        // Set verbose
        config.verbose = args.verbose;

        // If no stop conditions, default to 60 frames (1 second)
        if config.stop_conditions.is_empty() && !config.stop_on_halt {
            config.stop_conditions.push(StopCondition::Frames(60));
        }

        config
    }
}

impl SavestateConfig {
    /// Build savestate config from CLI arguments
    pub fn from_cli_args(args: &CliArgs) -> Self {
        let mut config = Self::new();

        // Load source
        if args.savestate.state_stdin {
            config.load_from = Some(SavestateSource::Stdin);
        } else if let Some(ref path) = args.savestate.load_state {
            config.load_from = Some(SavestateSource::File(path.clone()));
        }

        // Save destination
        if args.savestate.state_stdout {
            config.save_to = Some(SavestateDestination::Stdout);
        } else if let Some(ref path) = args.savestate.save_state {
            config.save_to = Some(SavestateDestination::File(path.clone()));
        }

        // Set format directly from CLI args (same type via re-export)
        config.format = args.savestate.state_format;

        config
    }
}
