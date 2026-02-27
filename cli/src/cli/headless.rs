//! Headless mode execution for the NES emulator CLI.
//!
//! This module handles all headless execution logic, including:
//! - ROM information display
//! - Memory initialization
//! - Video export (streaming and buffered modes)
//! - Screenshot capture
//! - Memory dump output

use std::path::Path;
use std::time::Instant;

use monsoon_core::emulation::nes::Nes;
use monsoon_core::emulation::palette_util::{RgbColor, RgbPalette};
use monsoon_core::emulation::ppu::{TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH};
use monsoon_core::emulation::rom::RomFile;

use crate::cli::{
    CliArgs, ExecutionConfig, ExecutionEngine, ExecutionResult, FpsConfig, MemoryDump, MemoryInit,
    MemoryInitConfig, MemoryType, OutputWriter, SavestateConfig, StopReason, StreamingVideoEncoder,
    VideoFormat, VideoResolution, apply_memory_init, apply_memory_init_config, is_ffmpeg_available,
    parse_memory_range,
};

// =============================================================================
// NES Constants
// =============================================================================

/// NES output width in pixels (256). Re-exported from core as u32 for image library APIs
/// which require u32 dimensions rather than usize.
pub const NES_WIDTH: u32 = TOTAL_OUTPUT_WIDTH as u32;

/// NES output height in pixels (240). Re-exported from core as u32 for image library APIs
/// which require u32 dimensions rather than usize.
pub const NES_HEIGHT: u32 = TOTAL_OUTPUT_HEIGHT as u32;

/// Convert palette indices (u16) to RGB colors using the default NES palette.
///
/// Each u16 value encodes: bits 0-5 = color index (0-63), bits 6-8 = emphasis bits.
pub fn palette_indices_to_rgb(indices: &[u16]) -> Vec<RgbColor> {
    let palette = RgbPalette::default();
    indices
        .iter()
        .map(|&idx| {
            let color = (idx as usize) & 0x3F;
            let emphasis = ((idx as usize) >> 6) & 0x07;
            palette.colors[emphasis][color]
        })
        .collect()
}

// =============================================================================
// Main Headless Entry Point
// =============================================================================

/// Run the emulator in headless mode.
///
/// This is the main entry point for headless CLI execution.
pub fn run_headless(args: &CliArgs) -> Result<(), String> {
    // Handle ROM info display (exits early)
    if args.rom.rom_info {
        return handle_rom_info(args);
    }

    let start = Instant::now();

    // Build execution and savestate configs from CLI args
    let exec_config = ExecutionConfig::from_cli_args(args);
    let savestate_config = SavestateConfig::from_cli_args(args);

    // Create and configure the execution engine
    let mut engine = ExecutionEngine::new()
        .with_config(exec_config)
        .with_savestate_config(savestate_config);

    // Load ROM
    if let Some(ref rom_path) = args.rom.rom {
        engine.load_rom(rom_path)?;
    }

    // Load savestate if configured
    engine.load_savestate()?;

    // Power on (unless --no-power is specified)
    if !args.power.no_power {
        engine.power_on();
    }

    // Handle reset
    if args.power.reset {
        engine.reset();
    }

    // Apply memory initialization (after power on, before execution)
    apply_memory_initialization(engine.emulator_mut(), args)?;

    // Determine if we should use streaming video export
    // Streaming mode: frames are written directly to encoder during execution
    // This significantly reduces memory usage for long recordings
    let use_streaming = args.video.video_path.is_some();

    let result = if use_streaming {
        run_with_streaming_video(&mut engine, args)?
    } else {
        // Standard buffered mode
        engine.run()?
    };

    // Print execution summary if verbose
    if args.verbose {
        eprintln!("Execution time: {:?}", start.elapsed());
        eprintln!("Total cycles: {}", result.total_cycles);
        eprintln!("Total frames: {}", result.total_frames);
        eprintln!("Stop reason: {:?}", result.stop_reason);
    }

    // Save savestate if configured
    engine.save_savestate()?;

    // Output memory dumps
    output_results(engine.emulator(), args)?;

    // Save screenshot (only if we have frames - in buffered mode)
    if !use_streaming {
        save_screenshot(&engine.frames, args)?;
    }

    // Video was already saved in streaming mode, skip in buffered mode if already done
    if !use_streaming {
        save_video(&engine.frames, args)?;
    }

    // Check for error stop reason
    match result.stop_reason {
        StopReason::Error(e) => Err(e),
        _ => Ok(()),
    }
}

// =============================================================================
// ROM Info
// =============================================================================

/// Handle --rom-info flag
fn handle_rom_info(args: &CliArgs) -> Result<(), String> {
    let rom_path = args
        .rom
        .rom
        .as_ref()
        .ok_or("--rom-info requires --rom to be specified")?;
    print_rom_info(rom_path)
}

/// Print ROM information to stdout.
pub fn print_rom_info(rom_path: &Path) -> Result<(), String> {
    let path_str = rom_path.to_string_lossy().to_string();
    let data = std::fs::read(rom_path)
        .map_err(|e| format!("Failed to read ROM file: {}", e))?;
    let rom = RomFile::load(&data, Some(path_str));

    println!("ROM Information:");
    println!("  File: {}", rom_path.display());
    if let Some(ref name) = rom.name {
        println!("  Name: {}", name);
    }
    println!("  Mapper: {}", rom.mapper_number);
    println!("  PRG ROM: {} KB", rom.prg_memory.prg_rom_size / 1024);
    println!("  CHR ROM: {} KB", rom.chr_memory.chr_rom_size / 1024);
    println!(
        "  PRG RAM: {} KB (battery-backed: {})",
        rom.prg_memory.prg_ram_size / 1024,
        if rom.is_battery_backed { "yes" } else { "no" }
    );
    println!(
        "  Mirroring: {}",
        if rom.hardwired_nametable_layout {
            "Vertical"
        } else {
            "Horizontal"
        }
    );
    println!(
        "  Checksum (SHA-256): {}",
        rom.data_checksum
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    );

    Ok(())
}

// =============================================================================
// Memory Initialization
// =============================================================================

/// Apply memory initialization based on CLI args
pub fn apply_memory_initialization(emu: &mut Nes, args: &CliArgs) -> Result<(), String> {
    // Parse CPU memory init commands
    let cpu_inits: Vec<MemoryInit> = args
        .memory
        .init_cpu
        .iter()
        .map(|s| MemoryInit::parse(s))
        .collect::<Result<Vec<_>, _>>()?;

    // Parse PPU memory init commands
    let ppu_inits: Vec<MemoryInit> = args
        .memory
        .init_ppu
        .iter()
        .map(|s| MemoryInit::parse(s))
        .collect::<Result<Vec<_>, _>>()?;

    // Parse OAM memory init commands
    let oam_inits: Vec<MemoryInit> = args
        .memory
        .init_oam
        .iter()
        .map(|s| MemoryInit::parse(s))
        .collect::<Result<Vec<_>, _>>()?;

    // Apply direct init commands
    if !cpu_inits.is_empty() || !ppu_inits.is_empty() || !oam_inits.is_empty() {
        apply_memory_init(emu, &cpu_inits, &ppu_inits, &oam_inits);
        if args.verbose {
            eprintln!(
                "Applied memory init: {} CPU, {} PPU, {} OAM operations",
                cpu_inits.len(),
                ppu_inits.len(),
                oam_inits.len()
            );
        }
    }

    // Load init config from file if specified
    if let Some(ref init_file) = args.memory.init_file {
        let config = MemoryInitConfig::load_from_file(init_file)?;
        apply_memory_init_config(emu, &config);
        if args.verbose {
            eprintln!(
                "Applied memory init from file: {} CPU, {} PPU, {} OAM addresses",
                config.cpu.len(),
                config.ppu.len(),
                config.oam.len()
            );
        }
    }

    Ok(())
}

// =============================================================================
// Streaming Video Export
// =============================================================================

/// Run emulation with streaming video export.
///
/// This mode writes frames directly to the encoder as they're generated,
/// instead of buffering all frames in memory. This significantly reduces
/// memory usage for long recordings.
fn run_with_streaming_video(
    engine: &mut ExecutionEngine,
    args: &CliArgs,
) -> Result<ExecutionResult, String> {
    let video_path = args.video.video_path.as_ref().unwrap();

    // Check if format requires FFmpeg and warn if not available
    if args.video.video_format == VideoFormat::Mp4 && !is_ffmpeg_available() {
        return Err("MP4 export requires FFmpeg to be installed. \
             Use --video-format png or --video-format ppm for self-contained export."
            .to_string());
    }

    // Parse video resolution
    let resolution = VideoResolution::parse(args.video.video_scale.as_ref().unwrap())
        .map_err(|e| format!("Invalid video scale: {}", e))?;

    // Parse FPS configuration
    let fps_config = FpsConfig::parse(&args.video.video_fps, args.video.video_mode)
        .map_err(|e| format!("Invalid video FPS: {}", e))?;

    let (dst_width, dst_height) = resolution.dimensions(NES_WIDTH, NES_HEIGHT);

    // Print export info
    if !args.quiet {
        print_video_info(
            video_path,
            &args.video.video_format,
            &resolution,
            NES_WIDTH,
            NES_HEIGHT,
            dst_width,
            dst_height,
            true,
            &fps_config,
        );
    }

    // Create streaming encoder with FPS config
    let mut encoder = StreamingVideoEncoder::with_fps_config(
        args.video.video_format,
        video_path,
        NES_WIDTH,
        NES_HEIGHT,
        &resolution,
        fps_config,
    )
    .map_err(|e| format!("Failed to create video encoder: {}", e))?;

    // Run with streaming video export
    let result = engine.run_with_video_encoder(&mut encoder)?;

    // Finalize encoder
    encoder
        .finish()
        .map_err(|e| format!("Failed to finalize video: {}", e))?;

    if !args.quiet {
        eprintln!("Exported {} frames successfully", encoder.frames_written());
    }

    // Handle screenshot in streaming mode (save last frame)
    if args.video.screenshot.is_some() {
        let last_frame = engine.emulator().get_pixel_buffer();
        let rgb_frame = palette_indices_to_rgb(&last_frame);
        save_single_screenshot(&rgb_frame, args)?;
    }

    Ok(result)
}

/// Print video export information
#[allow(clippy::too_many_arguments)]
fn print_video_info(
    video_path: &Path,
    format: &VideoFormat,
    resolution: &VideoResolution,
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    streaming: bool,
    fps_config: &FpsConfig,
) {
    let mode_str = if streaming { " [streaming mode]" } else { "" };
    let fps_str = format!(
        "{:.2} fps ({:?}, {}x)",
        fps_config.output_fps(),
        fps_config.mode,
        fps_config.multiplier
    );

    // Add note about mid-frame capture for multipliers > 1
    let capture_note = if fps_config.multiplier > 1 {
        format!(" ({} captures per PPU frame)", fps_config.multiplier)
    } else {
        String::new()
    };

    if *resolution == VideoResolution::Native {
        eprintln!(
            "Exporting to {} as {:?} ({}x{}, {}{}){}...",
            video_path.display(),
            format,
            src_width,
            src_height,
            fps_str,
            capture_note,
            mode_str
        );
    } else if matches!(format, VideoFormat::Mp4) {
        eprintln!(
            "Exporting to {} as {:?} ({}x{} → {}x{} via FFmpeg nearest-neighbor, {}{}){}...",
            video_path.display(),
            format,
            src_width,
            src_height,
            dst_width,
            dst_height,
            fps_str,
            capture_note,
            mode_str
        );
    } else {
        let scale_note = if streaming {
            " [streaming mode, scaling only supported for MP4]"
        } else {
            ""
        };
        eprintln!(
            "Exporting to {} as {:?} ({}x{}, {}{}){}...",
            video_path.display(),
            format,
            src_width,
            src_height,
            fps_str,
            capture_note,
            scale_note
        );
    }
}

// =============================================================================
// Screenshot Export
// =============================================================================

/// Save a single screenshot (used in streaming mode)
fn save_single_screenshot(frame: &[RgbColor], args: &CliArgs) -> Result<(), String> {
    if let Some(ref screenshot_path) = args.video.screenshot {
        let img: image::RgbaImage = image::ImageBuffer::from_fn(NES_WIDTH, NES_HEIGHT, |x, y| {
            let color = frame[(y * NES_WIDTH + x) as usize];
            image::Rgba([color.r, color.g, color.b, 255])
        });

        img.save(screenshot_path)
            .map_err(|e| format!("Failed to save screenshot: {}", e))?;

        if !args.quiet {
            eprintln!("Screenshot saved to {}", screenshot_path.display());
        }
    }
    Ok(())
}

/// Save screenshot to file from buffered frames
pub fn save_screenshot(frames: &[Vec<u16>], args: &CliArgs) -> Result<(), String> {
    if let Some(ref screenshot_path) = args.video.screenshot {
        if frames.is_empty() {
            eprintln!("Warning: No frames to screenshot");
            return Ok(());
        }

        // Use the last frame for screenshot
        let frame = frames.last().unwrap();
        let rgb_frame = palette_indices_to_rgb(frame);

        if !args.quiet {
            eprintln!("Saving screenshot to {}...", screenshot_path.display());
        }

        // Convert RgbColor to RGB bytes for PNG
        let rgb_data: Vec<u8> = rgb_frame.iter().flat_map(|c| [c.r, c.g, c.b]).collect();

        // Create PNG using image crate
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(NES_WIDTH, NES_HEIGHT, rgb_data)
                .ok_or_else(|| "Failed to create image buffer".to_string())?;

        img.save(screenshot_path)
            .map_err(|e| format!("Failed to save screenshot: {}", e))?;

        if !args.quiet {
            eprintln!("Screenshot saved successfully");
        }
    }

    Ok(())
}

// =============================================================================
// Buffered Video Export
// =============================================================================

/// Save recorded frames to video file
pub fn save_video(frames: &[Vec<u16>], args: &CliArgs) -> Result<(), String> {
    if let Some(ref video_path) = args.video.video_path {
        // Check if format requires FFmpeg and warn if not available
        if args.video.video_format == VideoFormat::Mp4 && !is_ffmpeg_available() {
            return Err("MP4 export requires FFmpeg to be installed. \
                 Use --video-format png or --video-format ppm for self-contained export."
                .to_string());
        }

        if frames.is_empty() {
            eprintln!("Warning: No frames to export");
            return Ok(());
        }

        // Parse video resolution
        let resolution = VideoResolution::parse(&args.video.video_scale.clone().unwrap())
            .map_err(|e| format!("Invalid video scale: {}", e))?;

        // Parse FPS configuration
        let fps_config = FpsConfig::parse(&args.video.video_fps, args.video.video_mode)
            .map_err(|e| format!("Invalid video FPS: {}", e))?;

        let (dst_width, dst_height) = resolution.dimensions(NES_WIDTH, NES_HEIGHT);

        if !args.quiet {
            eprintln!(
                "Exporting {} frames to {} as {:?} ({:.2} fps, {:?})...",
                frames.len(),
                video_path.display(),
                args.video.video_format,
                fps_config.output_fps(),
                fps_config.mode
            );
            if resolution != VideoResolution::Native && args.video.video_format == VideoFormat::Mp4
            {
                eprintln!(
                    "  Resolution: {}x{} → {}x{} via FFmpeg nearest-neighbor",
                    NES_WIDTH, NES_HEIGHT, dst_width, dst_height
                );
            }
        }

        // Use streaming encoder for proper scaling support
        let mut encoder = StreamingVideoEncoder::with_fps_config(
            args.video.video_format,
            video_path,
            NES_WIDTH,
            NES_HEIGHT,
            &resolution,
            fps_config,
        )
        .map_err(|e| format!("Failed to create video encoder: {}", e))?;

        for frame in frames {
            let rgb_frame = palette_indices_to_rgb(frame);
            encoder.write_frame(&rgb_frame).map_err(|e| e.to_string())?;
        }

        encoder.finish().map_err(|e| e.to_string())?;

        if !args.quiet {
            eprintln!("Exported {} frames successfully", encoder.frames_written());
        }
    }

    Ok(())
}

// =============================================================================
// Memory Dump Output
// =============================================================================

/// Output results based on CLI args using the output module abstraction.
pub fn output_results(emu: &Nes, args: &CliArgs) -> Result<(), String> {
    // Reset the output writer state for this run
    OutputWriter::reset();

    // Create the output writer with configured format and destination
    let writer = OutputWriter::new(args.output.output.clone(), args.output.effective_format());

    // Process each requested memory dump
    if let Some(ref range) = args.memory.read_cpu {
        let dump = create_cpu_dump(emu, range)?;
        writer.write(&dump)?;
    }

    if let Some(ref range) = args.memory.read_ppu {
        let dump = create_ppu_dump(emu, range)?;
        writer.write(&dump)?;
    }

    if args.memory.dump_oam {
        let dump = create_oam_dump(emu);
        writer.write(&dump)?;
    }

    if args.memory.dump_nametables {
        let dump = create_nametables_dump(emu);
        writer.write(&dump)?;
    }

    if args.memory.dump_palette {
        let dump = create_palette_dump(emu);
        writer.write(&dump)?;
    }

    Ok(())
}

// =============================================================================
// Memory Dump Creation
// =============================================================================

/// Create a CPU memory dump from the emulator
fn create_cpu_dump(emu: &Nes, range: &str) -> Result<MemoryDump, String> {
    let (start, end) = parse_memory_range(range)?;
    let mem = &emu.get_memory_debug(Some(start..=end))[0];
    Ok(MemoryDump::new(MemoryType::Cpu, start, mem.to_vec()))
}

/// Create a PPU memory dump from the emulator
fn create_ppu_dump(emu: &Nes, range: &str) -> Result<MemoryDump, String> {
    let (start, end) = parse_memory_range(range)?;
    let mem = &emu.get_memory_debug(Some(start..=end))[1];
    Ok(MemoryDump::new(MemoryType::Ppu, start, mem.to_vec()))
}

/// Create an OAM memory dump from the emulator
fn create_oam_dump(emu: &Nes) -> MemoryDump {
    let mem = emu.ppu.borrow().oam.get_memory_debug(None);
    MemoryDump::oam(mem)
}

/// Create a nametables memory dump from the emulator
fn create_nametables_dump(emu: &Nes) -> MemoryDump {
    let mem = emu
        .ppu
        .borrow()
        .memory
        .get_memory_debug(Some(0x2000..=0x2FFF));
    MemoryDump::nametables(mem)
}

/// Create a palette RAM memory dump from the emulator
fn create_palette_dump(emu: &Nes) -> MemoryDump {
    // Palette RAM is at PPU addresses $3F00-$3F1F (32 bytes)
    let mem = emu
        .ppu
        .borrow()
        .memory
        .get_memory_debug(Some(0x3F00..=0x3F1F));
    MemoryDump::palette_ram(mem)
}
