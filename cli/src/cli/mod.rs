//! CLI module for the NES emulator.
//!
//! This module provides a comprehensive command-line interface for programmatic
//! control of the emulator. It is designed with extensibility and robustness in mind.
//!
//! # Architecture
//!
//! The CLI is organized into several submodules, each with a specific responsibility:
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`args`] | Command-line argument definitions using clap derive macros |
//! | [`config`] | TOML configuration file support with merge logic |
//! | [`error`] | Comprehensive error types with helpful messages |
//! | [`execution`] | Execution engine with generic stop conditions |
//! | [`output`] | Extensible output formatting system |
//!
//! # Design Principles
//!
//! 1. **Separation of Concerns**: Each module has a single responsibility
//! 2. **Extensibility**: Adding new features requires minimal changes
//! 3. **Error Handling**: All errors are structured with helpful messages
//! 4. **Builder Pattern**: Configuration objects use fluent builder APIs
//! 5. **Crate-Ready**: All public types are designed for library use
//!
//! # Extensibility Guide
//!
//! ## Adding a New Output Format
//!
//! ```rust,ignore
//! // 1. Add variant to OutputFormat enum in args.rs
//! pub enum OutputFormat {
//!     Hex, Json, Toml, Binary,
//!     Xml,  // New!
//! }
//!
//! // 2. Implement MemoryFormatter trait in output.rs
//! pub struct XmlFormatter;
//! impl MemoryFormatter for XmlFormatter {
//!     fn format(&self, dump: &MemoryDump) -> Result<Vec<u8>, String> {
//!         // ... format as XML ...
//!     }
//!     fn file_extension(&self) -> &'static str { "xml" }
//! }
//!
//! // 3. Register in OutputFormat::formatter() and extension()
//! impl OutputFormat {
//!     pub fn formatter(&self) -> Box<dyn MemoryFormatter> {
//!         match self {
//!             // ... existing ...
//!             OutputFormat::Xml => Box::new(XmlFormatter),
//!         }
//!     }
//!     pub fn extension(&self) -> &'static str {
//!         match self {
//!             // ... existing ...
//!             OutputFormat::Xml => "xml",
//!         }
//!     }
//! }
//! ```
//!
//! ## Adding a New Stop Condition
//!
//! ```rust,ignore
//! // 1. Add variant to StopCondition enum in execution.rs
//! pub enum StopCondition {
//!     Cycles(u128), Frames(u64), PcEquals(u16),
//!     ScanlineEquals(u16),  // New!
//! }
//!
//! // 2. Add corresponding StopReason variant
//! pub enum StopReason {
//!     CyclesReached(u128), FramesReached(u64), PcReached(u16),
//!     ScanlineReached(u16),  // New!
//! }
//!
//! // 3. Implement check in ExecutionConfig::check_conditions()
//! StopCondition::ScanlineEquals(target) if emu.ppu.scanline == *target => {
//!     return Some(StopReason::ScanlineReached(*target));
//! }
//!
//! // 4. Add CLI argument in args.rs
//! #[arg(long)]
//! pub until_scanline: Option<u16>,
//!
//! // 5. Add builder method in ExecutionConfig
//! pub fn with_scanline(mut self, scanline: u16) -> Self {
//!     self.stop_conditions.push(StopCondition::ScanlineEquals(scanline));
//!     self
//! }
//! ```
//!
//! ## Adding a New Memory Type
//!
//! ```rust,ignore
//! // 1. Add variant to MemoryType enum in output.rs
//! pub enum MemoryType {
//!     Cpu, Ppu, Oam, Nametables,
//!     PaletteRam,  // New!
//! }
//!
//! // 2. Add factory method to MemoryDump
//! impl MemoryDump {
//!     pub fn palette_ram(data: Vec<u8>) -> Self {
//!         Self::new(MemoryType::PaletteRam, 0x3F00, data)
//!     }
//! }
//!
//! // 3. Add CLI argument in args.rs
//! #[arg(long)]
//! pub dump_palette_ram: bool,
//!
//! // 4. Handle in main.rs output_results()
//! if args.memory.dump_palette_ram {
//!     let dump = create_palette_ram_dump(emu);
//!     writer.write(&dump)?;
//! }
//! ```
//!
//! # Usage Examples
//!
//! ## Command Line
//!
//! ```bash
//! # Basic headless run
//! nes_main --headless --rom game.nes --frames 100
//!
//! # With config file
//! nes_main --config run.toml
//!
//! # Memory dump to file
//! nes_main -H --rom game.nes --frames 60 --read-cpu 0x0000-0x07FF --json -o memory.json
//!
//! # Pipe-based savestate workflow
//! nes_main -H --rom game.nes --frames 100 --state-stdout | \
//! nes_main -H --rom game.nes --state-stdin --frames 50 --save-state final.sav
//! ```
//!
//! ## Programmatic (Crate API)
//!
//! ```rust,ignore
//! use lockstep::cli::{ExecutionConfig, ExecutionEngine, SavestateConfig};
//! use std::path::PathBuf;
//!
//! // Create execution config with builder pattern
//! let exec_config = ExecutionConfig::new()
//!     .with_frames(100)
//!     .with_pc_breakpoint(0x8000)
//!     .with_verbose(true);
//!
//! // Create savestate config
//! let save_config = SavestateConfig::new()
//!     .save_to_file(PathBuf::from("output.sav"));
//!
//! // Run emulation
//! let mut engine = ExecutionEngine::new()
//!     .with_config(exec_config)
//!     .with_savestate_config(save_config);
//!
//! engine.load_rom(&PathBuf::from("game.nes"))?;
//! engine.power_on();
//!
//! let result = engine.run()?;
//! println!("Stopped: {:?} after {} frames", result.stop_reason, result.total_frames);
//!
//! engine.save_savestate()?;
//! ```
//!
//! See `docs/CLI_INTERFACE.md` for full documentation.

pub mod args;
pub mod config;
pub mod error;
pub mod execution;
pub mod headless;
pub mod memory_init;
pub mod output;
pub mod video;

pub use args::{
    CliArgs, OutputFormat, SavestateFormat, VideoExportMode, VideoFormat, parse_hex_u16,
};
use clap::Parser;
pub use config::ConfigFile;
pub use error::{CliError, CliResult};
pub use execution::{
    ExecutionConfig, ExecutionEngine, ExecutionResult, MemoryAccessType, SavestateConfig,
    SavestateDestination, SavestateSource, StopCondition, StopReason,
};
pub use headless::run_headless;
pub use memory_init::{MemoryInit, MemoryInitConfig, apply_memory_init, apply_memory_init_config};
pub use output::{
    InterpretedNametable, InterpretedNametables, InterpretedOam, MemoryDump, MemoryFormatter,
    MemoryType, OamSprite, OutputWriter,
};
pub use video::{
    FpsConfig, StreamingVideoEncoder, VideoEncoder, VideoError, VideoResolution, create_encoder,
    encode_frames, is_ffmpeg_available,
};

// =============================================================================
// Argument Parsing
// =============================================================================

/// Parse CLI arguments and optionally merge with a config file.
///
/// This function:
/// 1. Parses command-line arguments using clap
/// 2. If `--config` is specified, loads and merges the TOML config file
/// 3. Returns the final merged configuration
///
/// CLI arguments always take precedence over config file values.
pub fn parse_args() -> Result<CliArgs, Box<dyn std::error::Error>> {
    let mut args = CliArgs::parse();

    // If a config file is specified, load and merge it
    if let Some(ref config_path) = args.config {
        let config = ConfigFile::load(config_path)?;
        config.merge_with_cli(&mut args);
    }

    Ok(args)
}

// =============================================================================
// Argument Validation
// =============================================================================

/// Validate CLI arguments for consistency and completeness.
///
/// This function performs comprehensive validation of all CLI arguments,
/// checking for:
/// - Required arguments in certain modes
/// - Conflicting argument combinations
/// - Valid argument values
///
/// # Errors
///
/// Returns a structured `CliError` with helpful messages if validation fails.
///
/// # Example
///
/// ```rust,ignore
/// let args = parse_args()?;
/// validate_args(&args)?;  // Will error if args are invalid
/// ```
pub fn validate_args(args: &CliArgs) -> Result<(), CliError> {
    validate_headless_requirements(args)?;
    validate_savestate_options(args)?;
    validate_output_format(args)?;
    validate_memory_args(args)?;
    validate_execution_args(args)?;
    Ok(())
}

/// Validate that headless mode has required input.
fn validate_headless_requirements(args: &CliArgs) -> Result<(), CliError> {
    if args.rom.rom.is_none() && args.savestate.load_state.is_none() && !args.savestate.state_stdin
    {
        return Err(CliError::MissingArgument {
            arg: "--rom, --load-state, or --state-stdin".to_string(),
            context: "Headless mode requires an input source (ROM, savestate file, or stdin)"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate savestate argument combinations.
fn validate_savestate_options(args: &CliArgs) -> Result<(), CliError> {
    // Can't use both state-stdin and load-state
    if args.savestate.state_stdin && args.savestate.load_state.is_some() {
        return Err(CliError::conflicting_args(
            "--state-stdin",
            "--load-state",
            "can only load from one source at a time",
        ));
    }

    // Can't use both state-stdout and save-state
    if args.savestate.state_stdout && args.savestate.save_state.is_some() {
        return Err(CliError::conflicting_args(
            "--state-stdout",
            "--save-state",
            "can only save to one destination at a time",
        ));
    }

    Ok(())
}

/// Validate output format arguments.
fn validate_output_format(args: &CliArgs) -> Result<(), CliError> {
    let format_flags: Vec<&str> = [
        (args.output.json, "--json"),
        (args.output.toml, "--toml"),
        (args.output.binary, "--binary"),
    ]
    .iter()
    .filter_map(|(flag, name)| flag.then_some(*name))
    .collect();

    if format_flags.len() > 1 {
        return Err(CliError::InvalidArgumentCombination {
            args: format_flags.iter().map(|s| s.to_string()).collect(),
            reason: "can only specify one output format flag".to_string(),
        });
    }

    Ok(())
}

/// Validate memory-related arguments.
fn validate_memory_args(args: &CliArgs) -> Result<(), CliError> {
    // Validate CPU memory range if specified
    if let Some(ref range) = args.memory.read_cpu {
        validate_memory_range_syntax(range, "--read-cpu")?;
    }

    // Validate PPU memory range if specified
    if let Some(ref range) = args.memory.read_ppu {
        validate_memory_range_syntax(range, "--read-ppu")?;
    }

    Ok(())
}

/// Validate memory range syntax without parsing the actual values.
fn validate_memory_range_syntax(range: &str, arg_name: &str) -> Result<(), CliError> {
    // Must contain either '-' or ':'
    if !range.contains('-') && !range.contains(':') {
        return Err(CliError::invalid_arg_with_hint(
            arg_name,
            range,
            "invalid memory range format",
            "Use START-END (e.g., 0x0000-0x07FF) or START:LENGTH (e.g., 0x6000:0x100)",
        ));
    }
    Ok(())
}

/// Validate execution-related arguments.
fn validate_execution_args(args: &CliArgs) -> Result<(), CliError> {
    // Validate memory condition syntax if specified
    if let Some(ref cond) = args.execution.until_mem {
        validate_memory_condition_syntax(cond)?;
    }

    Ok(())
}

/// Validate memory condition syntax.
fn validate_memory_condition_syntax(cond: &Vec<String>) -> Result<(), CliError> {
    for s in cond {
        if !s.contains("==") && !s.contains("!=") {
            return Err(CliError::invalid_stop_condition(
                s,
                "missing comparison operator",
            ));
        }
    }
    Ok(())
}

// =============================================================================
// Memory Range Parsing
// =============================================================================

/// Parse a memory range string in format `START-END` or `START:LENGTH`.
///
/// Both `START` and `END`/`LENGTH` should be hexadecimal values (with or without 0x prefix).
///
/// # Errors
///
/// Returns an error if:
/// - The format is invalid (not START-END or START:LENGTH)
/// - The hex values cannot be parsed
/// - The resulting range would be invalid (end < start)
///
/// # Examples
///
/// ```
/// use monsoon_cli::cli::parse_memory_range;
///
/// assert_eq!(
///     parse_memory_range("0x0000-0x07FF").unwrap(),
///     (0x0000, 0x07FF)
/// );
/// assert_eq!(parse_memory_range("6000:100").unwrap(), (0x6000, 0x60FF));
/// ```
pub fn parse_memory_range(range: &str) -> Result<(u16, u16), String> {
    if let Some((start_str, end_str)) = range.split_once('-') {
        let start = parse_hex_u16(start_str)?;
        let end = parse_hex_u16(end_str)?;
        if end < start {
            return Err(format!(
                "Invalid memory range '{}': end address (0x{:04X}) is less than start (0x{:04X})",
                range, end, start
            ));
        }
        Ok((start, end))
    } else if let Some((start_str, len_str)) = range.split_once(':') {
        let start = parse_hex_u16(start_str)?;
        let len = parse_hex_u16(len_str)?;

        if len == 0 {
            return Err(format!(
                "Invalid memory range '{}': length cannot be zero",
                range
            ));
        }

        // Calculate end address, checking for overflow
        let end = start.checked_add(len.saturating_sub(1)).unwrap_or({
            // Overflow - clamp to max address
            0xFFFF
        });

        Ok((start, end))
    } else {
        Err(format!(
            "Invalid memory range format: '{}'. Use START-END or START:LENGTH (e.g., 0x0000-0x07FF or 0x6000:0x100)",
            range
        ))
    }
}
