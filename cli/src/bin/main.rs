//! NES Emulator CLI entry point.
//!
//! This is the main entry point for the NES emulator. It supports both
//! GUI mode (using egui) and headless mode for programmatic control.
//!
//! See `docs/CLI_INTERFACE.md` for full CLI documentation.

use std::process::ExitCode;

use monsoon_cli::cli::{parse_args, run_headless, validate_args};
// =============================================================================
// Exit Codes (as documented in CLI_INTERFACE.md)
// =============================================================================

const EXIT_SUCCESS: u8 = 0;
const EXIT_GENERAL_ERROR: u8 = 1;
const EXIT_INVALID_ARGS: u8 = 2;
// const EXIT_ROM_LOAD_FAILED: u8 = 3;  // Reserved for future use
// const EXIT_SAVESTATE_FAILED: u8 = 4; // Reserved for future use
// const EXIT_IO_ERROR: u8 = 5;         // Reserved for future use
// const EXIT_TIMEOUT: u8 = 6;          // Reserved for future use

// =============================================================================
// Main Entry Point
// =============================================================================

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Error parsing arguments: {}", e);
            return ExitCode::from(EXIT_INVALID_ARGS);
        }
    };

    // Skip full validation for list-renderers (it doesn't need ROM etc.)
    if args.video.list_renderers {
        let _ = run_headless(&args);
        return ExitCode::from(EXIT_SUCCESS);
    }

    if let Err(e) = validate_args(&args) {
        eprintln!("Invalid arguments: {}", e);
        return ExitCode::from(EXIT_INVALID_ARGS);
    }

    let result = run_headless(&args);

    match result {
        Ok(_) => {
            if !args.quiet {
                eprintln!("Emulator finished execution");
            }
            ExitCode::from(EXIT_SUCCESS)
        }
        Err(e) => {
            eprintln!("Emulator finished with error: \"{}\"", e);
            ExitCode::from(EXIT_GENERAL_ERROR)
        }
    }
}
