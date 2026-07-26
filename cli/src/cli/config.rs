//! Configuration file support for the NES emulator CLI.
//!
//! This module provides TOML configuration file parsing that can be merged
//! with command-line arguments. CLI arguments take precedence over config file
//! values.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use monsoon_core::util::{parse_hex_u8_opt, parse_hex_u16_opt};
use serde::Deserialize;

use crate::cli::args::BuiltinPalette;
use crate::cli::{CliArgs, OutputFormat, SavestateFormat, VideoExportMode, VideoFormat};

/// Default video FPS string value (1x multiplier)
pub const DEFAULT_VIDEO_FPS: &str = "1x";

/// TOML configuration file structure
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ConfigFile {
    /// Global options
    #[serde(default)]
    pub global: GlobalConfig,

    /// ROM configuration
    #[serde(default)]
    pub rom: RomConfig,

    /// Savestate configuration
    #[serde(default)]
    pub savestate: SavestateConfig,

    /// Memory configuration
    #[serde(default)]
    pub memory: MemoryConfig,

    /// Power configuration
    #[serde(default)]
    pub power: PowerConfig,

    /// Palette configuration
    #[serde(default)]
    pub palette: PaletteConfig,

    /// Video/screenshot configuration
    #[serde(default)]
    pub video: VideoConfig,

    /// Execution control configuration
    #[serde(default)]
    pub execution: ExecutionConfig,

    /// Output configuration
    #[serde(default)]
    pub output: OutputConfig,

    #[serde(default)]
    pub console_config: ConsoleConfig,
}

#[derive(Debug, Copy, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ConsoleConfig {
    pub alignment: Option<u8>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GlobalConfig {
    pub quiet: Option<bool>,
    pub verbose: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RomConfig {
    pub path: Option<PathBuf>,
    pub rom_info: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SavestateConfig {
    pub load: Option<PathBuf>,
    pub save: Option<PathBuf>,
    pub state_stdin: Option<bool>,
    pub state_stdout: Option<bool>,
    pub save_on: Option<String>,
    pub format: SavestateFormat,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub read_cpu: Option<String>,
    pub read_ppu: Option<String>,
    pub dump_oam: Option<bool>,
    pub dump_nametables: Option<bool>,
    pub dump_palette: Option<bool>,
    pub init_file: Option<PathBuf>,

    /// CPU memory initialization: address -> values
    #[serde(default)]
    pub init_cpu: HashMap<String, Vec<u8>>,

    /// PPU memory initialization: address -> values
    #[serde(default)]
    pub init_ppu: HashMap<String, Vec<u8>>,

    /// OAM memory initialization: address -> values
    #[serde(default)]
    pub init_oam: HashMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PowerConfig {
    pub no_power: Option<bool>,
    pub reset: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PaletteConfig {
    pub path: Option<PathBuf>,
    pub builtin: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct VideoConfig {
    pub screenshot: Option<PathBuf>,
    pub screenshot_on: Option<String>,
    pub video_path: Option<PathBuf>,
    pub video_format: Option<String>,
    /// Video FPS: Can be a multiplier like "2x", "3x" or a fixed value like
    /// "60.0"
    pub video_fps: Option<String>,
    /// Video export mode: "accurate" or "smooth"
    pub video_mode: Option<String>,
    pub video_scale: Option<String>,
    /// Screen renderer ID (e.g., "`PaletteLookup`")
    pub renderer: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExecutionConfig {
    pub cycles: Option<u64>,
    pub frames: Option<u64>,
    pub until_opcode: Option<String>,
    pub until_mem: Option<Vec<String>>,
    pub until_hlt: Option<bool>,
    pub trace: Option<PathBuf>,
    pub trace_log: Option<bool>,
    pub trace_log_path: Option<PathBuf>,
    #[serde(default)]
    pub breakpoints: Vec<String>,
    /// Memory watchpoints (format: "ADDR" or "ADDR:r" or "ADDR:w" or "ADDR:rw")
    #[serde(default)]
    pub watch_mem: Vec<String>,
    /// Alternative: `stop_conditions` as array of strings
    #[serde(default)]
    pub stop_conditions: Vec<String>,
    pub alignment: Option<u8>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub path: Option<PathBuf>,
    pub format: Option<String>,
    /// Shorthand for format = "json"
    pub json: Option<bool>,
    /// Shorthand for format = "toml"
    pub toml: Option<bool>,
    /// Shorthand for format = "binary"
    pub binary: Option<bool>,
}

impl ConfigFile {
    /// Load configuration from a TOML file
    pub fn load(path: &PathBuf) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path).map_err(|e| ConfigError::IoError(e.to_string()))?;
        toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))
    }

    /// Merge config file values with CLI arguments.
    /// CLI arguments take precedence over config file values.
    #[allow(clippy::too_many_lines)]
    pub fn merge_with_cli(&self, cli: &mut CliArgs) {
        if cli.console.alignment == 2 {
            cli.console.alignment = self.console_config.alignment.unwrap_or(2);
        }

        // Global options
        if !cli.quiet {
            cli.quiet = self.global.quiet.unwrap_or(false);
        }
        if !cli.verbose {
            cli.verbose = self.global.verbose.unwrap_or(false);
        }

        // ROM options
        if cli.rom.rom.is_none() {
            cli.rom.rom.clone_from(&self.rom.path);
        }
        if !cli.rom.rom_info {
            cli.rom.rom_info = self.rom.rom_info.unwrap_or(false);
        }

        // Savestate options
        if cli.savestate.load_state.is_none() {
            cli.savestate.load_state.clone_from(&self.savestate.load);
        }
        if cli.savestate.save_state.is_none() {
            cli.savestate.save_state.clone_from(&self.savestate.save);
        }
        if !cli.savestate.state_stdin {
            cli.savestate.state_stdin = self.savestate.state_stdin.unwrap_or(false);
        }
        if !cli.savestate.state_stdout {
            cli.savestate.state_stdout = self.savestate.state_stdout.unwrap_or(false);
        }
        if cli.savestate.save_state_on.is_none() {
            cli.savestate
                .save_state_on
                .clone_from(&self.savestate.save_on);
        }
        if cli.savestate.state_format == SavestateFormat::Binary {
            cli.savestate.state_format = self.savestate.format;
        }

        // Memory options
        if cli.memory.read_cpu.is_none() {
            cli.memory.read_cpu.clone_from(&self.memory.read_cpu);
        }
        if cli.memory.read_ppu.is_none() {
            cli.memory.read_ppu.clone_from(&self.memory.read_ppu);
        }
        if !cli.memory.dump_oam {
            cli.memory.dump_oam = self.memory.dump_oam.unwrap_or(false);
        }
        if !cli.memory.dump_nametables {
            cli.memory.dump_nametables = self.memory.dump_nametables.unwrap_or(false);
        }
        if !cli.memory.dump_palette {
            cli.memory.dump_palette = self.memory.dump_palette.unwrap_or(false);
        }
        if cli.memory.init_file.is_none() {
            cli.memory.init_file.clone_from(&self.memory.init_file);
        }
        // Merge init_cpu from config if CLI has none
        if cli.memory.init_cpu.is_empty() {
            for (addr, values) in &self.memory.init_cpu {
                let values_str = values
                    .iter()
                    .map(|v| format!("0x{v:02X}"))
                    .collect::<Vec<_>>()
                    .join(",");
                cli.memory.init_cpu.push(format!("{addr}={values_str}"));
            }
        }
        // Merge init_ppu from config if CLI has none
        if cli.memory.init_ppu.is_empty() {
            for (addr, values) in &self.memory.init_ppu {
                let values_str = values
                    .iter()
                    .map(|v| format!("0x{v:02X}"))
                    .collect::<Vec<_>>()
                    .join(",");
                cli.memory.init_ppu.push(format!("{addr}={values_str}"));
            }
        }
        // Merge init_oam from config if CLI has none
        if cli.memory.init_oam.is_empty() {
            for (addr, values) in &self.memory.init_oam {
                let values_str = values
                    .iter()
                    .map(|v| format!("0x{v:02X}"))
                    .collect::<Vec<_>>()
                    .join(",");
                cli.memory.init_oam.push(format!("{addr}={values_str}"));
            }
        }

        // Power options
        if !cli.power.no_power {
            cli.power.no_power = self.power.no_power.unwrap_or(false);
        }
        if !cli.power.reset {
            cli.power.reset = self.power.reset.unwrap_or(false);
        }

        // Palette options
        if cli.palette.palette.is_none() {
            cli.palette.palette.clone_from(&self.palette.path);
        }
        if cli.palette.palette_builtin.is_none()
            && let Some(ref builtin) = self.palette.builtin
        {
            cli.palette.palette_builtin = BuiltinPalette::from_str(builtin).ok();
        }

        // Video options
        if cli.video.screenshot.is_none() {
            cli.video.screenshot.clone_from(&self.video.screenshot);
        }
        if cli.video.screenshot_on.is_none() {
            cli.video
                .screenshot_on
                .clone_from(&self.video.screenshot_on);
        }
        if cli.video.video_path.is_none() {
            cli.video.video_path.clone_from(&self.video.video_path);
        }
        if let Some(ref fmt) = self.video.video_format {
            // Only override if CLI is at default
            if cli.video.video_format == VideoFormat::Raw {
                cli.video.video_format = VideoFormat::from_str(fmt).unwrap_or(VideoFormat::Raw);
            }
        }
        if cli.video.video_scale.is_none() {
            if self.video.video_scale.is_none() {
                cli.video.video_scale = Some("native".to_string());
            } else {
                cli.video.video_scale.clone_from(&self.video.video_scale);
            }
        }
        // Only override video_fps if CLI is at default
        if cli.video.video_fps == DEFAULT_VIDEO_FPS
            && let Some(ref fps) = self.video.video_fps
        {
            cli.video.video_fps.clone_from(fps);
        }
        // Only override video_mode if CLI is at default
        if cli.video.video_mode == VideoExportMode::Accurate
            && let Some(ref mode) = self.video.video_mode
        {
            cli.video.video_mode =
                VideoExportMode::from_str(mode).unwrap_or(VideoExportMode::Accurate);
        }
        // Renderer option
        if cli.video.renderer.is_none() {
            cli.video.renderer.clone_from(&self.video.renderer);
        }

        // Execution options
        if cli.execution.cycles.is_none() {
            cli.execution.cycles = self.execution.cycles;
        }
        if cli.execution.frames.is_none() {
            cli.execution.frames = self.execution.frames;
        }
        if cli.execution.until_opcode.is_none()
            && let Some(ref op) = self.execution.until_opcode
        {
            cli.execution.until_opcode = parse_hex_u8_opt(op);
        }
        if cli.execution.until_mem.is_none() {
            cli.execution
                .until_mem
                .clone_from(&self.execution.until_mem);
        }
        if !cli.execution.until_hlt {
            cli.execution.until_hlt = self.execution.until_hlt.unwrap_or(false);
        }
        if cli.execution.trace.is_none() {
            cli.execution.trace.clone_from(&self.execution.trace);
        }
        if !cli.execution.trace_log {
            cli.execution.trace_log = self.execution.trace_log.unwrap_or(false);
        }
        if cli.execution.trace_log_path.is_none() {
            cli.execution
                .trace_log_path
                .clone_from(&self.execution.trace_log_path);
        }
        if cli.execution.breakpoint.is_empty() {
            for bp in &self.execution.breakpoints {
                if let Some(addr) = parse_hex_u16_opt(bp) {
                    cli.execution.breakpoint.push(addr);
                }
            }
        }
        if cli.execution.watch_mem.is_empty() {
            cli.execution
                .watch_mem
                .clone_from(&self.execution.watch_mem);
        }

        // Parse stop_conditions into appropriate fields
        for cond in &self.execution.stop_conditions {
            if let Some(rest) = cond.strip_prefix("pc:") {
                // pc: condition is now handled as a breakpoint
                if let Some(addr) = parse_hex_u16_opt(rest) {
                    cli.execution.breakpoint.push(addr);
                }
            } else if let Some(rest) = cond.strip_prefix("frames:") {
                if cli.execution.frames.is_none() {
                    cli.execution.frames = rest.parse().ok();
                }
            } else if let Some(rest) = cond.strip_prefix("cycles:")
                && cli.execution.cycles.is_none()
            {
                cli.execution.cycles = rest.parse().ok();
            }
        }

        // Output options
        if cli.output.output.is_none() {
            cli.output.output.clone_from(&self.output.path);
        }
        // Handle shorthand flags from config (precedence: json > toml > binary >
        // format) This matches the CLI behavior in OutputArgs::effective_format()
        if !cli.output.json && self.output.json.unwrap_or(false) {
            cli.output.json = true;
        }
        if !cli.output.toml && self.output.toml.unwrap_or(false) {
            cli.output.toml = true;
        }
        if !cli.output.binary && self.output.binary.unwrap_or(false) {
            cli.output.binary = true;
        }
        if let Some(ref fmt) = self.output.format {
            // Only override if CLI is at default and no shorthand flags
            if cli.output.output_format == OutputFormat::Hex
                && !cli.output.json
                && !cli.output.toml
                && !cli.output.binary
            {
                cli.output.output_format = OutputFormat::from_str(fmt).unwrap_or(OutputFormat::Hex);
            }
        }
    }
}

/// Configuration loading errors
#[derive(Debug, Clone)]
pub enum ConfigError {
    IoError(String),
    ParseError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "Failed to read config file: {e}"),
            ConfigError::ParseError(e) => write!(f, "Failed to parse config file: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_config() {
        let config: ConfigFile = toml::from_str("").unwrap();
        assert!(config.rom.path.is_none());
    }

    #[test]
    fn test_parse_basic_config() {
        let toml_str = r#"
            [rom]
            path = "game.nes"

            [execution]
            frames = 100
        "#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(config.rom.path, Some(PathBuf::from("game.nes")));
        assert_eq!(config.execution.frames, Some(100));
    }

    #[test]
    fn test_parse_memory_init() {
        let toml_str = r#"
            [memory.init_cpu]
            "0x0050" = [0xFF, 0x00, 0x10]
            "0x0060" = [0x01]
        "#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.memory.init_cpu.get("0x0050"),
            Some(&vec![0xFF, 0x00, 0x10])
        );
        assert_eq!(config.memory.init_cpu.get("0x0060"), Some(&vec![0x01]));
    }

    #[test]
    fn test_parse_stop_conditions() {
        let toml_str = r#"
            [execution]
            stop_conditions = ["pc:0x8500", "frames:3600"]
        "#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(config.execution.stop_conditions.len(), 2);
    }
}
