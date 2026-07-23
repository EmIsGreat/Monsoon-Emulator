//! Error types for the CLI module.
//!
//! This module provides a comprehensive error type system for the CLI,
//! enabling precise error handling and helpful error messages.
//!
//! # Design Goals
//!
//! - **Structured errors**: Each error type captures context for debugging
//! - **Helpful messages**: Error display includes suggestions when possible
//! - **Easy matching**: Errors can be matched on to handle specific cases
//! - **Composable**: Errors can wrap underlying causes
//!
//! # Example
//!
//! ```rust,ignore
//! use lockstep::cli::error::{CliError, CliResult};
//!
//! fn load_config(path: &str) -> CliResult<Config> {
//!     let content = std::fs::read_to_string(path)
//!         .map_err(|e| CliError::config_io(path, e))?;
//!
//!     toml::from_str(&content)
//!         .map_err(|e| CliError::config_parse(path, e))
//! }
//! ```

use std::clone::Clone;
use std::fmt;
use std::path::PathBuf;

/// Result type alias for CLI operations.
pub type CliResult<T> = Result<T, CliError>;

/// Comprehensive error type for CLI operations.
///
/// This enum covers all possible error conditions in the CLI module,
/// providing structured information for error handling and reporting.
#[derive(Debug, Clone)]
pub enum CliError {
    // =========================================================================
    // Argument Errors
    // =========================================================================
    /// Invalid argument value
    InvalidArgument {
        arg: String,
        value: String,
        reason: String,
        hint: Option<String>,
    },

    /// Missing required argument
    MissingArgument { arg: String, context: String },

    /// Conflicting arguments specified
    ConflictingArguments {
        arg1: String,
        arg2: String,
        reason: String,
    },

    /// Invalid combination of arguments
    InvalidArgumentCombination { args: Vec<String>, reason: String },

    // =========================================================================
    // Config File Errors
    // =========================================================================
    /// Failed to read config file
    ConfigIo { path: PathBuf, message: String },

    /// Failed to parse config file
    ConfigParse {
        path: PathBuf,
        message: String,
        line: Option<usize>,
    },

    /// Invalid config value
    ConfigValue {
        path: PathBuf,
        key: String,
        value: String,
        reason: String,
    },

    // =========================================================================
    // ROM Errors
    // =========================================================================
    /// Failed to load ROM
    RomLoad { path: PathBuf, message: String },

    /// ROM not found
    RomNotFound { path: PathBuf },

    /// Invalid ROM format
    RomInvalid { path: PathBuf, reason: String },

    // =========================================================================
    // Savestate Errors
    // =========================================================================
    /// Failed to load savestate
    SavestateLoad { source: String, message: String },

    /// Failed to save savestate
    SavestateSave {
        destination: String,
        message: String,
    },

    /// Invalid savestate format
    SavestateInvalid { source: String, reason: String },

    // =========================================================================
    // Memory Errors
    // =========================================================================
    /// Invalid memory address
    InvalidAddress { address: String, reason: String },

    /// Invalid memory range
    InvalidMemoryRange {
        range: String,
        reason: String,
        hint: Option<String>,
    },

    /// Memory access error
    MemoryAccess {
        operation: String,
        address: u16,
        message: String,
    },

    // =========================================================================
    // Execution Errors
    // =========================================================================
    /// Execution error
    Execution {
        message: String,
        cycles: Option<u64>,
        frames: Option<u64>,
    },

    /// Invalid stop condition
    InvalidStopCondition {
        condition: String,
        reason: String,
        hint: Option<String>,
    },

    // =========================================================================
    // Output Errors
    // =========================================================================
    /// Failed to write output
    OutputWrite {
        destination: String,
        message: String,
    },

    /// Invalid output format
    InvalidOutputFormat {
        format: String,
        valid_formats: Vec<String>,
    },

    // =========================================================================
    // Generic Errors
    // =========================================================================
    /// I/O error
    Io { operation: String, message: String },

    /// Internal error (should not happen)
    Internal { message: String },
}

impl CliError {
    // =========================================================================
    // Argument Error Constructors
    // =========================================================================

    /// Create an invalid argument error.
    pub fn invalid_arg(
        arg: impl Into<String>,
        value: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidArgument {
            arg: arg.into(),
            value: value.into(),
            reason: reason.into(),
            hint: None,
        }
    }

    /// Create an invalid argument error with a hint.
    pub fn invalid_arg_with_hint(
        arg: impl Into<String>,
        value: impl Into<String>,
        reason: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self::InvalidArgument {
            arg: arg.into(),
            value: value.into(),
            reason: reason.into(),
            hint: Some(hint.into()),
        }
    }

    /// Create a missing argument error.
    pub fn missing_arg(arg: impl Into<String>, context: impl Into<String>) -> Self {
        Self::MissingArgument {
            arg: arg.into(),
            context: context.into(),
        }
    }

    /// Create a conflicting arguments error.
    pub fn conflicting_args(
        arg1: impl Into<String>,
        arg2: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::ConflictingArguments {
            arg1: arg1.into(),
            arg2: arg2.into(),
            reason: reason.into(),
        }
    }

    // =========================================================================
    // Config Error Constructors
    // =========================================================================

    /// Create a config I/O error.
    pub fn config_io(path: impl Into<PathBuf>, err: impl fmt::Display) -> Self {
        Self::ConfigIo {
            path: path.into(),
            message: err.to_string(),
        }
    }

    /// Create a config parse error.
    pub fn config_parse(path: impl Into<PathBuf>, err: impl fmt::Display) -> Self {
        Self::ConfigParse {
            path: path.into(),
            message: err.to_string(),
            line: None,
        }
    }

    // =========================================================================
    // Memory Error Constructors
    // =========================================================================

    /// Create an invalid memory range error.
    pub fn invalid_memory_range(range: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidMemoryRange {
            range: range.into(),
            reason: reason.into(),
            hint: Some(
                "Use START-END (e.g., 0x0000-0x07FF) or START:LENGTH (e.g., 0x6000:0x100)".into(),
            ),
        }
    }

    /// Create an invalid address error.
    pub fn invalid_address(address: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidAddress {
            address: address.into(),
            reason: reason.into(),
        }
    }

    // =========================================================================
    // Savestate Error Constructors
    // =========================================================================

    /// Create a savestate load error.
    pub fn savestate_load(source: impl Into<String>, err: impl fmt::Display) -> Self {
        Self::SavestateLoad {
            source: source.into(),
            message: err.to_string(),
        }
    }

    /// Create a savestate save error.
    pub fn savestate_save(destination: impl Into<String>, err: impl fmt::Display) -> Self {
        Self::SavestateSave {
            destination: destination.into(),
            message: err.to_string(),
        }
    }

    // =========================================================================
    // Execution Error Constructors
    // =========================================================================

    /// Create an execution error.
    pub fn execution(message: impl Into<String>) -> Self {
        Self::Execution {
            message: message.into(),
            cycles: None,
            frames: None,
        }
    }

    /// Create an invalid stop condition error.
    pub fn invalid_stop_condition(condition: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidStopCondition {
            condition: condition.into(),
            reason: reason.into(),
            hint: Some("Valid formats: ADDR==VALUE, ADDR!=VALUE (e.g., 0x6000==0x80)".into()),
        }
    }

    // =========================================================================
    // Output Error Constructors
    // =========================================================================

    /// Create an output write error.
    pub fn output_write(destination: impl Into<String>, err: impl fmt::Display) -> Self {
        Self::OutputWrite {
            destination: destination.into(),
            message: err.to_string(),
        }
    }

    // =========================================================================
    // Generic Error Constructors
    // =========================================================================

    /// Create an I/O error.
    pub fn io(operation: impl Into<String>, err: impl fmt::Display) -> Self {
        Self::Io {
            operation: operation.into(),
            message: err.to_string(),
        }
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    // =========================================================================
    // Error Classification
    // =========================================================================

    /// Check if this is a user error (invalid input).
    #[must_use]
    pub fn is_user_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidArgument { .. }
                | Self::MissingArgument { .. }
                | Self::ConflictingArguments { .. }
                | Self::InvalidArgumentCombination { .. }
                | Self::ConfigParse { .. }
                | Self::ConfigValue { .. }
                | Self::InvalidAddress { .. }
                | Self::InvalidMemoryRange { .. }
                | Self::InvalidStopCondition { .. }
                | Self::InvalidOutputFormat { .. }
        )
    }

    /// Check if this is an I/O error.
    #[must_use]
    pub fn is_io_error(&self) -> bool {
        matches!(
            self,
            Self::ConfigIo { .. }
                | Self::RomLoad { .. }
                | Self::RomNotFound { .. }
                | Self::SavestateLoad { .. }
                | Self::SavestateSave { .. }
                | Self::OutputWrite { .. }
                | Self::Io { .. }
        )
    }

    /// Get the exit code for this error.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            // Invalid arguments
            Self::InvalidArgument {
                ..
            }
            | Self::MissingArgument {
                ..
            }
            | Self::ConflictingArguments {
                ..
            }
            | Self::InvalidArgumentCombination {
                ..
            } => 2,

            // ROM errors
            Self::RomLoad {
                ..
            }
            | Self::RomNotFound {
                ..
            }
            | Self::RomInvalid {
                ..
            } => 3,

            // Savestate errors
            Self::SavestateLoad {
                ..
            }
            | Self::SavestateSave {
                ..
            }
            | Self::SavestateInvalid {
                ..
            } => 4,

            // I/O errors
            Self::ConfigIo {
                ..
            }
            | Self::OutputWrite {
                ..
            }
            | Self::Io {
                ..
            } => 5,

            // Other errors
            _ => 1,
        }
    }
}

impl fmt::Display for CliError {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Argument errors
            Self::InvalidArgument {
                arg,
                value,
                reason,
                hint,
            } => {
                write!(
                    f,
                    "Invalid value '{value}' for argument '{arg}': {reason}"
                )?;
                if let Some(h) = hint {
                    write!(f, "\nHint: {h}")?;
                }
                Ok(())
            }
            Self::MissingArgument {
                arg,
                context,
            } => {
                write!(f, "Missing required argument '{arg}': {context}")
            }
            Self::ConflictingArguments {
                arg1,
                arg2,
                reason,
            } => {
                write!(
                    f,
                    "Cannot use '{arg1}' and '{arg2}' together: {reason}"
                )
            }
            Self::InvalidArgumentCombination {
                args,
                reason,
            } => {
                write!(
                    f,
                    "Invalid argument combination [{}]: {}",
                    args.join(", "),
                    reason
                )
            }

            // Config errors
            Self::ConfigIo {
                path,
                message,
            } => {
                write!(
                    f,
                    "Failed to read config file '{}': {}",
                    path.display(),
                    message
                )
            }
            Self::ConfigParse {
                path,
                message,
                line,
            } => {
                write!(f, "Failed to parse config file '{}'", path.display())?;
                if let Some(l) = line {
                    write!(f, " at line {l}")?;
                }
                write!(f, ": {message}")
            }
            Self::ConfigValue {
                path,
                key,
                value,
                reason,
            } => {
                write!(
                    f,
                    "Invalid value '{}' for key '{}' in config '{}': {}",
                    value,
                    key,
                    path.display(),
                    reason
                )
            }

            // ROM errors
            Self::RomLoad {
                path,
                message,
            } => {
                write!(f, "Failed to load ROM '{}': {}", path.display(), message)
            }
            Self::RomNotFound {
                path,
            } => {
                write!(f, "ROM file not found: {}", path.display())
            }
            Self::RomInvalid {
                path,
                reason,
            } => {
                write!(f, "Invalid ROM file '{}': {}", path.display(), reason)
            }

            // Savestate errors
            Self::SavestateLoad {
                source,
                message,
            } => {
                write!(f, "Failed to load savestate from {source}: {message}")
            }
            Self::SavestateSave {
                destination,
                message,
            } => {
                write!(
                    f,
                    "Failed to save savestate to {destination}: {message}"
                )
            }
            Self::SavestateInvalid {
                source,
                reason,
            } => {
                write!(f, "Invalid savestate from {source}: {reason}")
            }

            // Memory errors
            Self::InvalidAddress {
                address,
                reason,
            } => {
                write!(f, "Invalid address '{address}': {reason}")
            }
            Self::InvalidMemoryRange {
                range,
                reason,
                hint,
            } => {
                write!(f, "Invalid memory range '{range}': {reason}")?;
                if let Some(h) = hint {
                    write!(f, "\nHint: {h}")?;
                }
                Ok(())
            }
            Self::MemoryAccess {
                operation,
                address,
                message,
            } => {
                write!(
                    f,
                    "Memory {operation} at 0x{address:04X} failed: {message}"
                )
            }

            // Execution errors
            Self::Execution {
                message,
                cycles,
                frames,
            } => {
                write!(f, "Execution error: {message}")?;
                if let Some(c) = cycles {
                    write!(f, " (after {c} cycles")?;
                    if let Some(fr) = frames {
                        write!(f, ", {fr} frames")?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Self::InvalidStopCondition {
                condition,
                reason,
                hint,
            } => {
                write!(f, "Invalid stop condition '{condition}': {reason}")?;
                if let Some(h) = hint {
                    write!(f, "\nHint: {h}")?;
                }
                Ok(())
            }

            // Output errors
            Self::OutputWrite {
                destination,
                message,
            } => {
                write!(f, "Failed to write output to {destination}: {message}")
            }
            Self::InvalidOutputFormat {
                format,
                valid_formats,
            } => {
                write!(
                    f,
                    "Invalid output format '{}'. Valid formats: {}",
                    format,
                    valid_formats.join(", ")
                )
            }

            // Generic errors
            Self::Io {
                operation,
                message,
            } => {
                write!(f, "I/O error during {operation}: {message}")
            }
            Self::Internal {
                message,
            } => {
                write!(f, "Internal error: {message}")
            }
        }
    }
}

impl std::error::Error for CliError {}

// =========================================================================
// Conversions
// =========================================================================

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            operation: "I/O operation".into(),
            message: err.to_string(),
        }
    }
}

impl From<toml::de::Error> for CliError {
    fn from(err: toml::de::Error) -> Self {
        Self::ConfigParse {
            path: PathBuf::new(),
            message: err.to_string(),
            // Note: TOML errors don't provide line numbers directly,
            // the error message itself contains location info
            line: None,
        }
    }
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CliError::invalid_arg("--frames", "-5", "must be positive");
        assert!(err.to_string().contains("--frames"));
        assert!(err.to_string().contains("-5"));
        assert!(err.to_string().contains("positive"));
    }

    #[test]
    fn test_error_with_hint() {
        let err = CliError::invalid_arg_with_hint(
            "--until-pc",
            "invalid",
            "not a valid hex address",
            "Use format 0xADDR or ADDR",
        );
        let msg = err.to_string();
        assert!(msg.contains("Hint:"));
    }

    #[test]
    fn test_conflicting_args() {
        let err = CliError::conflicting_args(
            "--state-stdin",
            "--load-state",
            "can only use one input source",
        );
        assert!(err.to_string().contains("--state-stdin"));
        assert!(err.to_string().contains("--load-state"));
    }

    #[test]
    fn test_exit_codes() {
        assert_eq!(CliError::invalid_arg("a", "b", "c").exit_code(), 2);
        assert_eq!(
            CliError::RomNotFound {
                path: PathBuf::new()
            }
            .exit_code(),
            3
        );
        assert_eq!(CliError::savestate_load("file", "err").exit_code(), 4);
        assert_eq!(CliError::io("read", "err").exit_code(), 5);
    }

    #[test]
    fn test_error_classification() {
        let user_err = CliError::invalid_arg("a", "b", "c");
        let io_err = CliError::io("read", "failed");

        assert!(user_err.is_user_error());
        assert!(!user_err.is_io_error());

        assert!(!io_err.is_user_error());
        assert!(io_err.is_io_error());
    }

    #[test]
    fn test_memory_range_error() {
        let err = CliError::invalid_memory_range("invalid", "bad format");
        let msg = err.to_string();
        assert!(msg.contains("invalid"));
        assert!(msg.contains("Hint:"));
        assert!(msg.contains("START-END"));
    }
}
