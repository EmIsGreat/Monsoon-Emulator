use std::process::ExitCode;

use clap::value_parser;

#[cfg(feature = "default_renderers")]
mod renderers {
    extern crate monsoon_default_renderers;
}

const EXIT_SUCCESS: u8 = 0;
const EXIT_GENERAL_ERROR: u8 = 1;
// const EXIT_ROM_LOAD_FAILED: u8 = 3;  // Reserved for future use
// const EXIT_SAVESTATE_FAILED: u8 = 4; // Reserved for future use
// const EXIT_IO_ERROR: u8 = 5;         // Reserved for future use
// const EXIT_TIMEOUT: u8 = 6;          // Reserved for future use

use std::path::PathBuf;

use clap::Parser;
use monsoon_frontend::frontend::egui_frontend;
#[derive(Parser, Debug, Clone, Default)]
#[command(name = "Monsoon-Emulator - Frontend")]
#[command(version, about, long_about = None)]
pub struct CliArgs {
    /// Path to ROM file
    #[arg(short, long, value_parser = value_parser!(PathBuf), value_hint = clap::ValueHint::FilePath
    )]
    pub rom: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = CliArgs::parse();

    let result = run_gui(&args);

    match result {
        Ok(_) => {
            eprintln!("Emulator finished execution");
            ExitCode::from(EXIT_SUCCESS)
        }
        Err(e) => {
            eprintln!("Emulator finished with error: \"{}\"", e);
            ExitCode::from(EXIT_GENERAL_ERROR)
        }
    }
}

// =============================================================================
// GUI Mode
// =============================================================================

fn run_gui(args: &CliArgs) -> Result<(), String> {
    // Renderer type is selected at runtime via RendererKind
    // The frontend uses RendererKind which can be switched dynamically
    egui_frontend::run(args.rom.clone()).map_err(|e| e.to_string())
}
