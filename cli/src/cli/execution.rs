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

use monsoon_core::emulation::debug_tools::{MemoryAccessType, StopCondition, StopReason};
use monsoon_core::emulation::nes::{
    ExecutionResult, Nes, NesConfig, RunOptions, MASTER_CYCLES_PER_FRAME,
};
use monsoon_core::emulation::rom::{ParseError, RomFile};
use monsoon_core::emulation::savestate::{try_load_state_from_bytes, SaveState};
use monsoon_core::util::{SerializationError, ToBytes};

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
    pub alignment: u8,
}

/// Default output path for internal trace logs when tracing is enabled without
/// an explicit output path.
const DEFAULT_INTERNAL_TRACE_LOG_PATH: &str = "trace.log";

impl ExecutionConfig {
    /// Create a new empty execution config
    pub fn new() -> Self { Self::default() }

    /// Add a stop condition
    pub fn with_stop_condition(mut self, condition: StopCondition) -> Self {
        self.stop_conditions.push(condition);
        self
    }

    /// Set stop after N cycles
    pub fn with_cycles(mut self, cycles: u64) -> Self {
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
    fn max_cycles(&self) -> u64 {
        let mut max = u64::MAX;
        for cond in &self.stop_conditions {
            match cond {
                StopCondition::Cycles(c) => max = max.min(*c),
                StopCondition::Frames(f) => {
                    max = max.min(*f * MASTER_CYCLES_PER_FRAME as u64)
                }
                _ => {}
            }
        }
        max
    }
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

pub use crate::cli::args::SavestateFormat;
// Re-export SavestateFormat from args for use in this module
use crate::cli::CliArgs;

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
    pub fn new(nes_config: NesConfig) -> Self {
        Self {
            emu: Nes::with_config(nes_config),
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
        let loadable: Result<RomFile, ParseError> = (&path_str, true, Some(&self.emu)).try_into();

        match loadable {
            Ok(rom) => {
                let _ = self.emu.load_rom(&rom);
                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
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
            let encoded = encode_savestate(&state, self.savestate_config.format);

            return match encoded {
                Ok(data) => {
                    match dest {
                        SavestateDestination::File(path) => {
                            std::fs::write(path, &data).map_err(|e| {
                                format!("Failed to write savestate to {}: {}", path.display(), e)
                            })?;
                        }
                        SavestateDestination::Stdout => {
                            std::io::stdout().write_all(&data).map_err(|e| {
                                format!("Failed to write savestate to stdout: {}", e)
                            })?;
                        }
                    }

                    Ok(())
                }
                Err(err) => Err(err.to_string()),
            };
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

        // Run frame by frame for stop condition checking
        let result = loop {
            // Run one frame
            match self.emu.step_frame() {
                Ok(_) => {}
                Err(e) => {
                    break ExecutionResult {
                        last_cycle_reached: false,
                        hlt_reached: false,
                        cycle_completed: false,
                        cpu_cycle_completed: false,
                        ppu_cycle_completed: false,
                        frame_done: false,
                        scanline_done: false,
                        stop_reason: Some(StopReason::Error(e)),
                    };
                }
            }

            // Only collect frames if in buffered mode
            if self.collect_frames {
                self.frames.push(self.emu.get_pixel_buffer().to_vec());
            }

            self.frame_count += 1;

            // Check stop conditions
            if let Some(reason) = self.emu.check_stop_conditions(&self.config.stop_conditions) {
                break ExecutionResult {
                    last_cycle_reached: false,
                    hlt_reached: false,
                    cycle_completed: false,
                    cpu_cycle_completed: false,
                    ppu_cycle_completed: false,
                    frame_done: false,
                    scanline_done: false,
                    stop_reason: Some(reason),
                };
            }

            // Check max cycles
            if self.emu.total_cycles >= max_cycles {
                break ExecutionResult {
                    last_cycle_reached: false,
                    hlt_reached: false,
                    cycle_completed: false,
                    cpu_cycle_completed: false,
                    ppu_cycle_completed: false,
                    frame_done: false,
                    scanline_done: false,
                    stop_reason: None,
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
                let odd_frame_offset: i32 = if self.emu.is_even_frame() && self.emu.is_rendering() {
                    2
                } else {
                    -2
                };

                let base = (capture_idx + 1) as u64 * MASTER_CYCLES_PER_FRAME as u64;

                let base = if odd_frame_offset >= 0 {
                    base.saturating_add(odd_frame_offset as u64)
                } else {
                    base.saturating_sub((-odd_frame_offset) as u64)
                };

                let capture_point = base / captures_per_frame as u64;
                let target_cycles = frame_start_cycles + capture_point;

                // Run until the target cycle
                match self.emu.run_until(target_cycles, RunOptions::default()) {
                    Ok(_) => {}
                    Err(e) => {
                        return Ok(ExecutionResult {
                            last_cycle_reached: false,
                            hlt_reached: false,
                            cycle_completed: false,
                            cpu_cycle_completed: false,
                            ppu_cycle_completed: false,
                            frame_done: false,
                            scanline_done: false,
                            stop_reason: Some(StopReason::Error(e)),
                        });
                    }
                }

                // Write frame directly to encoder (with upscaling if configured)
                // This captures the current pixel buffer state, which may be mid-render
                let frame = self.emu.get_pixel_buffer();
                let rgb_frame = renderer.buffer_to_image(frame);
                encoder
                    .write_frame(rgb_frame)
                    .map_err(|e| format!("Video encoding error: {}", e))?;

                // Only increment frame_count at the end of a full PPU frame
                // (when we've done all captures for this frame)
                if capture_idx == captures_per_frame - 1 {
                    self.frame_count += 1;
                }
            }

            // Check stop conditions
            if let Some(reason) = self.emu.check_stop_conditions(&self.config.stop_conditions) {
                self.write_trace_log()?;
                return Ok(ExecutionResult {
                    last_cycle_reached: false,
                    hlt_reached: false,
                    cycle_completed: false,
                    cpu_cycle_completed: false,
                    ppu_cycle_completed: false,
                    frame_done: false,
                    scanline_done: false,
                    stop_reason: Some(reason),
                });
            }

            // Check max cycles
            if self.emu.total_cycles >= max_cycles {
                self.write_trace_log()?;
                return Ok(ExecutionResult {
                    last_cycle_reached: false,
                    hlt_reached: false,
                    cycle_completed: false,
                    cpu_cycle_completed: false,
                    ppu_cycle_completed: false,
                    frame_done: false,
                    scanline_done: false,
                    stop_reason: None,
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
    fn default() -> Self { Self::new(NesConfig::default()) }
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
fn encode_savestate(
    state: &SaveState,
    format: SavestateFormat,
) -> Result<Vec<u8>, SerializationError> {
    match format {
        SavestateFormat::Binary => state.to_bytes(None),
        SavestateFormat::Json => state.to_bytes(Some("json".to_string())),
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
        config.trace_path = args
            .execution
            .trace_log_path
            .clone()
            .or_else(|| args.execution.trace.clone());

        if args.execution.trace_log && config.trace_path.is_none() {
            config.trace_path = Some(PathBuf::from(DEFAULT_INTERNAL_TRACE_LOG_PATH));
        }

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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{ExecutionConfig, DEFAULT_INTERNAL_TRACE_LOG_PATH};
    use crate::cli::CliArgs;

    #[test]
    fn from_cli_args_enables_internal_log_with_default_path() {
        let mut args = CliArgs::default();
        args.execution.trace_log = true;

        let config = ExecutionConfig::from_cli_args(&args);
        assert_eq!(
            config.trace_path,
            Some(PathBuf::from(DEFAULT_INTERNAL_TRACE_LOG_PATH))
        );
    }

    #[test]
    fn from_cli_args_internal_log_path_takes_precedence_with_logging_disabled() {
        let mut args = CliArgs::default();
        args.execution.trace_log = false;
        args.execution.trace_log_path = Some(PathBuf::from("internal.log"));
        args.execution.trace = Some(PathBuf::from("legacy.log"));

        let config = ExecutionConfig::from_cli_args(&args);
        assert_eq!(config.trace_path, Some(PathBuf::from("internal.log")));
    }

    #[test]
    fn from_cli_args_supports_legacy_trace_path() {
        let mut args = CliArgs::default();
        args.execution.trace = Some(PathBuf::from("legacy.log"));

        let config = ExecutionConfig::from_cli_args(&args);
        assert_eq!(config.trace_path, Some(PathBuf::from("legacy.log")));
    }
}
