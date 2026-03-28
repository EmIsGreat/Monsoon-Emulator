//! Video export module for the NES emulator CLI.
//!
//! This module provides video encoding capabilities for exporting emulator
//! frame buffers to various video formats.
//!
//! # Supported Formats
//!
//! | Format | Implementation | Dependencies |
//! |--------|---------------|--------------|
//! | PNG sequence | Pure Rust (image crate) | None - self-contained |
//! | PPM sequence | Pure Rust | None - self-contained |
//! | MP4 | FFmpeg subprocess | FFmpeg installed |
//! | Raw | Writes raw BGRA bytes | None |
//!
//! # Scaling
//!
//! Video scaling is handled natively by FFmpeg using the nearest neighbor
//! filter, which preserves sharp pixel edges for retro games.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use image::{ImageBuffer, Rgba, RgbaImage};
use monsoon_core::emulation::palette_util::RgbColor;

use crate::cli::args::VideoFormat;

// =============================================================================
// Video Resolution
// =============================================================================

/// Target video resolution for output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoResolution {
    /// Native resolution (256x240 for NES)
    Native,
    /// Integer scale (2x, 3x, 4x, etc.)
    IntegerScale(u32),
    /// 720p (1280x720) - fit within bounds
    Hd720,
    /// 1080p (1920x1080) - fit within bounds
    Hd1080,
    /// 4K (3840x2160) - fit within bounds
    Uhd4k,
    /// Custom resolution
    Custom(u32, u32),
}

impl VideoResolution {
    /// Parse a video resolution string.
    ///
    /// Supported formats:
    /// - "native" - Native resolution
    /// - "2x", "3x", "4x" - Integer scales
    /// - "720p", "1080p", "4k" - Standard resolutions
    /// - "WxH" or "WIDTHxHEIGHT" - Custom resolution (e.g., "1920x1080")
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.to_lowercase();
        match s.as_str() {
            "native" | "1x" => Ok(VideoResolution::Native),
            "2x" => Ok(VideoResolution::IntegerScale(2)),
            "3x" => Ok(VideoResolution::IntegerScale(3)),
            "4x" => Ok(VideoResolution::IntegerScale(4)),
            "5x" => Ok(VideoResolution::IntegerScale(5)),
            "6x" => Ok(VideoResolution::IntegerScale(6)),
            "720p" | "hd" => Ok(VideoResolution::Hd720),
            "1080p" | "fullhd" | "fhd" => Ok(VideoResolution::Hd1080),
            "4k" | "uhd" | "2160p" => Ok(VideoResolution::Uhd4k),
            _ => {
                // Try to parse as WxH
                if let Some((w, h)) = s.split_once('x') {
                    let width = w
                        .trim()
                        .parse()
                        .map_err(|_| format!("Invalid width: {}", w))?;
                    let height = h
                        .trim()
                        .parse()
                        .map_err(|_| format!("Invalid height: {}", h))?;
                    Ok(VideoResolution::Custom(width, height))
                } else {
                    Err(format!(
                        "Unknown resolution: '{}'. Try: native, 2x, 3x, 4x, 720p, 1080p, 4k, or \
                         WxH",
                        s
                    ))
                }
            }
        }
    }

    /// Calculate the output dimensions for a given source size.
    ///
    /// For preset resolutions (720p, 1080p, 4k), the output is scaled to fit
    /// within the target while maintaining aspect ratio with the NES PAR (8:7).
    pub fn dimensions(&self, src_width: u32, src_height: u32) -> (u32, u32) {
        // NES pixel aspect ratio: 8:7 (pixels are slightly wider than tall)
        const NES_PAR: f64 = 8.0 / 7.0;

        match self {
            VideoResolution::Native => (src_width, src_height),
            VideoResolution::IntegerScale(scale) => (src_width * scale, src_height * scale),
            VideoResolution::Hd720 => fit_to_bounds(src_width, src_height, 1280, 720, NES_PAR),
            VideoResolution::Hd1080 => fit_to_bounds(src_width, src_height, 1920, 1080, NES_PAR),
            VideoResolution::Uhd4k => fit_to_bounds(src_width, src_height, 3840, 2160, NES_PAR),
            VideoResolution::Custom(w, h) => (*w, *h),
        }
    }
}

/// Fit source dimensions to target bounds while maintaining aspect ratio.
fn fit_to_bounds(
    src_width: u32,
    src_height: u32,
    max_width: u32,
    max_height: u32,
    par: f64,
) -> (u32, u32) {
    // Calculate the maximum integer scale that fits within bounds
    let scale_x = max_width as f64 / (src_width as f64 * par);
    let scale_y = max_height as f64 / src_height as f64;
    let scale = scale_x.min(scale_y);

    // Use integer scale for clean pixel scaling
    let int_scale = scale.floor() as u32;
    let int_scale = int_scale.max(1); // At least 1x

    // Calculate output dimensions
    let out_width = (src_width as f64 * par * int_scale as f64).round() as u32;
    let out_height = src_height * int_scale;

    // Ensure dimensions are even (required for many video codecs)
    let out_width = (out_width + 1) & !1;
    let out_height = (out_height + 1) & !1;

    (out_width, out_height)
}

// =============================================================================
// FPS Configuration
// =============================================================================

use crate::cli::args::VideoExportMode;

/// NES NTSC framerate: 39375000 / 655171 ≈ 60.098814
pub const NES_NTSC_FPS: f64 = 39375000.0 / 655171.0;

/// NES NTSC framerate as exact numerator/denominator
pub const NES_NTSC_FPS_NUM: u64 = 39375000;
pub const NES_NTSC_FPS_DEN: u64 = 655171;

/// Smooth framerate target (exactly 60 fps)
pub const SMOOTH_FPS: f64 = 60.0;

/// Parsed FPS configuration.
///
/// This struct handles parsing of FPS strings (like "1x", "2x", "120") and
/// calculates the appropriate capture rate and output framerate based on
/// the video export mode.
#[derive(Debug, Clone)]
pub struct FpsConfig {
    /// Multiplier for frame capture (1 = normal, 2 = capture twice per frame,
    /// etc.)
    pub multiplier: u32,
    /// The export mode (accurate or smooth)
    pub mode: VideoExportMode,
}

impl FpsConfig {
    /// Parse an FPS string (e.g., "1x", "2x", "60", "120.0").
    ///
    /// - Multipliers like "1x", "2x", "3x" specify how often to sample the
    ///   framebuffer
    /// - Fixed values like "60" or "120.0" are converted to the nearest
    ///   multiplier
    pub fn parse(s: &str, mode: VideoExportMode) -> Result<Self, String> {
        let s = s.trim().to_lowercase();

        // Try parsing as multiplier (e.g., "1x", "2x", "3x")
        if let Some(mult_str) = s.strip_suffix('x') {
            let multiplier: u32 = mult_str
                .parse()
                .map_err(|_| format!("Invalid FPS multiplier: '{}'", s))?;
            if multiplier == 0 {
                return Err("FPS multiplier must be at least 1".to_string());
            }
            return Ok(Self {
                multiplier,
                mode,
            });
        }

        // Try parsing as a fixed FPS value
        let fps: f64 = s.parse().map_err(|_| {
            format!(
                "Invalid FPS value: '{}'. Use multipliers like '2x' or fixed values like '60.0'",
                s
            )
        })?;

        if fps <= 0.0 {
            return Err("FPS must be positive".to_string());
        }

        // Convert fixed FPS to multiplier based on mode
        let base_fps = match mode {
            VideoExportMode::Accurate => NES_NTSC_FPS,
            VideoExportMode::Smooth => SMOOTH_FPS,
        };

        // Calculate multiplier (round to nearest integer)
        let multiplier = (fps / base_fps).round() as u32;
        let multiplier = multiplier.max(1);

        Ok(Self {
            multiplier,
            mode,
        })
    }

    /// Get the output framerate as a floating-point value.
    pub fn output_fps(&self) -> f64 {
        match self.mode {
            VideoExportMode::Accurate => NES_NTSC_FPS * self.multiplier as f64,
            VideoExportMode::Smooth => SMOOTH_FPS * self.multiplier as f64,
        }
    }

    /// Get the output framerate as a rational string for FFmpeg.
    ///
    /// For accurate mode, this returns the exact NES framerate fraction
    /// multiplied. For smooth mode, this returns clean integer multiples of
    /// 60.
    pub fn output_fps_rational(&self) -> String {
        match self.mode {
            VideoExportMode::Accurate => {
                // Use exact rational: (39375000 * multiplier) / 655171
                let numerator = NES_NTSC_FPS_NUM * self.multiplier as u64;
                format!("{}/{}", numerator, NES_NTSC_FPS_DEN)
            }
            VideoExportMode::Smooth => {
                // Clean integer FPS
                let fps = 60 * self.multiplier;
                format!("{}/1", fps)
            }
        }
    }

    /// Get the number of frames to capture per PPU frame.
    ///
    /// For 1x, this is 1 (capture once per complete frame).
    /// For 2x, this is 2 (capture at mid-frame and end of frame).
    /// For 3x, this is 3 (capture at 1/3, 2/3, and end of frame).
    pub fn captures_per_frame(&self) -> u32 { self.multiplier }

    /// Check if this configuration requires mid-frame captures.
    pub fn needs_mid_frame_capture(&self) -> bool { self.multiplier > 1 }
}

impl Default for FpsConfig {
    fn default() -> Self {
        Self {
            multiplier: 1,
            mode: VideoExportMode::Accurate,
        }
    }
}

// =============================================================================
// Error Types
// =============================================================================

/// Video encoding error
#[derive(Debug)]
pub enum VideoError {
    /// FFmpeg is not installed or not found in PATH
    FfmpegNotFound,
    /// FFmpeg process failed
    FfmpegFailed(String),
    /// I/O error
    IoError(io::Error),
    /// Image encoding error
    ImageError(String),
    /// Invalid frame dimensions
    InvalidDimensions {
        expected: (u32, u32),
        got: (u32, u32),
    },
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoError::FfmpegNotFound => {
                write!(
                    f,
                    "FFmpeg not found. Please install FFmpeg for MP4 export, or use PNG/PPM \
                     format."
                )
            }
            VideoError::FfmpegFailed(msg) => write!(f, "FFmpeg encoding failed: {}", msg),
            VideoError::IoError(e) => write!(f, "I/O error: {}", e),
            VideoError::ImageError(e) => write!(f, "Image encoding error: {}", e),
            VideoError::InvalidDimensions {
                expected,
                got,
            } => {
                write!(
                    f,
                    "Invalid frame dimensions: expected {}x{}, got {}x{}",
                    expected.0, expected.1, got.0, got.1
                )
            }
        }
    }
}

impl std::error::Error for VideoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VideoError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for VideoError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::NotFound {
            VideoError::FfmpegNotFound
        } else {
            VideoError::IoError(e)
        }
    }
}

impl From<image::ImageError> for VideoError {
    fn from(e: image::ImageError) -> Self { VideoError::ImageError(e.to_string()) }
}

// =============================================================================
// Video Encoder Trait
// =============================================================================

/// Trait for video encoders.
///
/// Implement this trait to add support for new video formats.
pub trait VideoEncoder: Send {
    /// Write a frame to the video.
    ///
    /// The pixel buffer is in RGB format as (R, G, B) tuples,
    /// as a flat array of width × height values.
    fn write_frame(&mut self, pixel_buffer: &[RgbColor]) -> Result<(), VideoError>;

    /// Finish encoding and flush any remaining data.
    fn finish(&mut self) -> Result<(), VideoError>;

    /// Get the number of frames written so far.
    fn frames_written(&self) -> u64;
}

// =============================================================================
// Factory Function
// =============================================================================

/// Create a video encoder for the specified format.
///
/// For MP4 format with scaling, use `create_encoder_with_scale` instead.
pub fn create_encoder(
    format: VideoFormat,
    output_path: &Path,
    width: u32,
    height: u32,
    fps: f64,
) -> Result<Box<dyn VideoEncoder>, VideoError> {
    match format {
        VideoFormat::Png => Ok(Box::new(PngSequenceEncoder::new(
            output_path,
            width,
            height,
        )?)),
        VideoFormat::Ppm => Ok(Box::new(PpmSequenceEncoder::new(
            output_path,
            width,
            height,
        )?)),
        VideoFormat::Mp4 => Ok(Box::new(FfmpegMp4Encoder::new(
            output_path,
            width,
            height,
            fps,
            None,
        )?)),
        VideoFormat::Raw => Ok(Box::new(RawEncoder::new(width, height)?)),
    }
}

/// Create an MP4 encoder with FFmpeg native nearest-neighbor scaling.
///
/// This passes scaling to FFmpeg using `-vf scale=W:H:flags=neighbor`,
/// which is efficient and produces sharp pixel edges.
pub fn create_encoder_with_scale(
    output_path: &Path,
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    fps: f64,
) -> Result<Box<dyn VideoEncoder>, VideoError> {
    Ok(Box::new(FfmpegMp4Encoder::new(
        output_path,
        src_width,
        src_height,
        fps,
        Some((dst_width, dst_height)),
    )?))
}

// =============================================================================
// PNG Sequence Encoder
// =============================================================================

/// Encoder that outputs a sequence of PNG images.
pub struct PngSequenceEncoder {
    base_path: PathBuf,
    width: u32,
    height: u32,
    frame_count: u64,
}

impl PngSequenceEncoder {
    /// Create a new PNG sequence encoder.
    pub fn new(output_path: &Path, width: u32, height: u32) -> Result<Self, VideoError> {
        if let Some(parent) = output_path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }

        Ok(Self {
            base_path: output_path.to_path_buf(),
            width,
            height,
            frame_count: 0,
        })
    }

    fn frame_path(&self, frame: u64) -> PathBuf {
        let stem = self
            .base_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "frame".to_string());
        let dir = self
            .base_path
            .parent()
            .unwrap_or(Path::new("../../../../../../.."));
        dir.join(format!("{}_{:06}.png", stem, frame))
    }
}

impl VideoEncoder for PngSequenceEncoder {
    fn write_frame(&mut self, pixel_buffer: &[RgbColor]) -> Result<(), VideoError> {
        let expected_size = (self.width * self.height) as usize;
        if pixel_buffer.len() != expected_size {
            let got_height = pixel_buffer.len() / self.width as usize;
            return Err(VideoError::InvalidDimensions {
                expected: (self.width, self.height),
                got: (self.width, got_height as u32),
            });
        }

        let img: RgbaImage = ImageBuffer::from_fn(self.width, self.height, |x, y| {
            let color = pixel_buffer[(y * self.width + x) as usize];
            Rgba([color.r, color.g, color.b, 255])
        });

        let path = self.frame_path(self.frame_count);
        img.save(&path)?;

        self.frame_count += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), VideoError> { Ok(()) }

    fn frames_written(&self) -> u64 { self.frame_count }
}

// =============================================================================
// PPM Sequence Encoder
// =============================================================================

/// Encoder that outputs a sequence of PPM images.
pub struct PpmSequenceEncoder {
    base_path: PathBuf,
    width: u32,
    height: u32,
    frame_count: u64,
}

impl PpmSequenceEncoder {
    /// Create a new PPM sequence encoder.
    pub fn new(output_path: &Path, width: u32, height: u32) -> Result<Self, VideoError> {
        if let Some(parent) = output_path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }

        Ok(Self {
            base_path: output_path.to_path_buf(),
            width,
            height,
            frame_count: 0,
        })
    }

    fn frame_path(&self, frame: u64) -> PathBuf {
        let stem = self
            .base_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "frame".to_string());
        let dir = self
            .base_path
            .parent()
            .unwrap_or(Path::new("../../../../../../.."));
        dir.join(format!("{}_{:06}.ppm", stem, frame))
    }
}

impl VideoEncoder for PpmSequenceEncoder {
    fn write_frame(&mut self, pixel_buffer: &[RgbColor]) -> Result<(), VideoError> {
        let expected_size = (self.width * self.height) as usize;
        if pixel_buffer.len() != expected_size {
            let got_height = pixel_buffer.len() / self.width as usize;
            return Err(VideoError::InvalidDimensions {
                expected: (self.width, self.height),
                got: (self.width, got_height as u32),
            });
        }

        let path = self.frame_path(self.frame_count);
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);

        writeln!(writer, "P6")?;
        writeln!(writer, "{} {}", self.width, self.height)?;
        writeln!(writer, "255")?;

        for &RgbColor {
            r,
            g,
            b,
        } in pixel_buffer
        {
            writer.write_all(&[r, g, b])?;
        }

        writer.flush()?;
        self.frame_count += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), VideoError> { Ok(()) }

    fn frames_written(&self) -> u64 { self.frame_count }
}

// =============================================================================
// FFmpeg MP4 Encoder
// =============================================================================

/// Encoder that pipes frames to FFmpeg for MP4 encoding.
///
/// Supports native nearest-neighbor scaling via FFmpeg's scale filter.
pub struct FfmpegMp4Encoder {
    child: Option<Child>,
    stdin: Option<BufWriter<std::process::ChildStdin>>,
    stderr_path: Option<PathBuf>,
    width: u32,
    height: u32,
    frame_count: u64,
}

impl FfmpegMp4Encoder {
    /// Create a new FFmpeg MP4 encoder.
    ///
    /// If `scale_to` is provided, FFmpeg will scale to that resolution using
    /// nearest-neighbor interpolation (sharp pixel edges).
    pub fn new(
        output_path: &Path,
        width: u32,
        height: u32,
        fps: f64,
        scale_to: Option<(u32, u32)>,
    ) -> Result<Self, VideoError> {
        // Check if ffmpeg exists
        let ffmpeg_check = Command::new("ffmpeg").arg("-version").output();

        match ffmpeg_check {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(VideoError::FfmpegNotFound);
            }
            Err(e) => return Err(VideoError::IoError(e)),
            Ok(_) => {}
        }

        // Create output directory if needed
        if let Some(parent) = output_path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }

        let stderr_path =
            std::env::temp_dir().join(format!("nes_ffmpeg_stderr_{}.log", std::process::id()));
        let stderr_file = File::create(&stderr_path)?;

        let path = output_path.with_extension("mp4");

        // Convert FPS to a precise fractional representation for FFmpeg.
        // This avoids frame timing drift caused by floating-point approximations.
        // For the NES NTSC framerate (39375000/655171 ≈ 60.0988), we use the exact
        // fraction.
        let fps_str = fps_to_rational(fps);

        // Build FFmpeg arguments
        let mut args = vec![
            "-y".to_string(),
            "-f".to_string(),
            "rawvideo".to_string(),
            "-pixel_format".to_string(),
            "bgra".to_string(),
            "-video_size".to_string(),
            format!("{}x{}", width, height),
            "-framerate".to_string(),
            fps_str,
            "-i".to_string(),
            "-".to_string(),
        ];

        // Add scaling filter if requested (nearest neighbor for sharp pixels)
        if let Some((dst_w, dst_h)) = scale_to
            && (dst_w != width || dst_h != height)
        {
            eprintln!(
                "FFmpeg scaling {}x{} -> {}x{} (nearest neighbor)",
                width, height, dst_w, dst_h
            );
            args.extend([
                "-vf".to_string(),
                format!("scale={}:{}:flags=neighbor", dst_w, dst_h),
            ]);
        }

        // Encoder settings
        args.extend([
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "fast".to_string(),
            "-crf".to_string(),
            "16".to_string(),
            "-vsync".to_string(),
            "cfr".to_string(),
            "-video_track_timescale".to_string(),
            "39375000".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            "-f".to_string(),
            "mp4".to_string(),
            path.to_str().unwrap_or("output.mp4").to_string(),
        ]);

        let mut child = Command::new("ffmpeg")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr_file))
            .spawn()
            .map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    VideoError::FfmpegNotFound
                } else {
                    VideoError::IoError(e)
                }
            })?;

        let stdin = child.stdin.take().map(BufWriter::new);

        Ok(Self {
            child: Some(child),
            stdin,
            stderr_path: Some(stderr_path),
            width,
            height,
            frame_count: 0,
        })
    }
}

impl VideoEncoder for FfmpegMp4Encoder {
    fn write_frame(&mut self, pixel_buffer: &[RgbColor]) -> Result<(), VideoError> {
        let expected_size = (self.width * self.height) as usize;
        if pixel_buffer.len() != expected_size {
            let got_height = pixel_buffer.len() / self.width as usize;
            return Err(VideoError::InvalidDimensions {
                expected: (self.width, self.height),
                got: (self.width, got_height as u32),
            });
        }

        // Convert RGB to BGRA
        let mut bgra_buffer = Vec::with_capacity(pixel_buffer.len() * 4);
        for &RgbColor {
            r,
            g,
            b,
        } in pixel_buffer
        {
            bgra_buffer.extend_from_slice(&[b, g, r, 255]);
        }

        match &mut self.stdin {
            Some(stdin) => {
                stdin.write_all(&bgra_buffer).map_err(|e| {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        VideoError::FfmpegFailed(
                            "FFmpeg closed input pipe unexpectedly".to_string(),
                        )
                    } else {
                        VideoError::IoError(e)
                    }
                })?;
            }
            None => {
                return Err(VideoError::FfmpegFailed(
                    "FFmpeg stdin not available".to_string(),
                ));
            }
        }

        self.frame_count += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), VideoError> {
        // Close stdin to signal EOF to ffmpeg
        self.stdin.take();

        // Wait for ffmpeg to finish
        if let Some(mut child) = self.child.take() {
            let status = child.wait()?;
            if !status.success() {
                let stderr_content = if let Some(ref path) = self.stderr_path {
                    fs::read_to_string(path).unwrap_or_default()
                } else {
                    String::new()
                };
                return Err(VideoError::FfmpegFailed(format!(
                    "FFmpeg exited with status {}: {}",
                    status,
                    stderr_content
                        .lines()
                        .take(10)
                        .collect::<Vec<_>>()
                        .join("\n")
                )));
            }
        }

        // Clean up stderr file
        if let Some(ref path) = self.stderr_path {
            let _ = fs::remove_file(path);
        }

        Ok(())
    }

    fn frames_written(&self) -> u64 { self.frame_count }
}

impl Drop for FfmpegMp4Encoder {
    fn drop(&mut self) {
        // Ensure stdin is closed
        self.stdin.take();

        // Try to wait for child process
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }

        // Clean up stderr file
        if let Some(ref path) = self.stderr_path {
            let _ = fs::remove_file(path);
        }
    }
}

// =============================================================================
// Raw Encoder
// =============================================================================

/// Encoder that outputs raw BGRA frames.
pub struct RawEncoder {
    width: u32,
    height: u32,
    frame_count: u64,
    stdout: BufWriter<io::Stdout>,
}

impl RawEncoder {
    /// Create a new raw encoder.
    pub fn new(width: u32, height: u32) -> Result<Self, VideoError> {
        Ok(Self {
            width,
            height,
            frame_count: 0,
            stdout: BufWriter::new(io::stdout()),
        })
    }
}

impl VideoEncoder for RawEncoder {
    fn write_frame(&mut self, pixel_buffer: &[RgbColor]) -> Result<(), VideoError> {
        let expected_size = (self.width * self.height) as usize;
        if pixel_buffer.len() != expected_size {
            let got_height = pixel_buffer.len() / self.width as usize;
            return Err(VideoError::InvalidDimensions {
                expected: (self.width, self.height),
                got: (self.width, got_height as u32),
            });
        }

        // Convert RGB to BGRA and write to stdout
        for &RgbColor {
            r,
            g,
            b,
        } in pixel_buffer
        {
            self.stdout.write_all(&[b, g, r, 255])?;
        }

        self.frame_count += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), VideoError> {
        self.stdout.flush()?;
        Ok(())
    }

    fn frames_written(&self) -> u64 { self.frame_count }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Convert FPS to a rational string representation for FFmpeg.
///
/// This function converts floating-point FPS values to precise fractional
/// representations to avoid frame timing drift in video encoding.
///
/// Known framerates (like NES NTSC 60.0988 FPS) are converted to their
/// exact rational form. Other values use a high-precision approximation.
fn fps_to_rational(fps: f64) -> String {
    // Tolerance values:
    // - NES NTSC: 0.01 because the irrational framerate may have rounding errors
    // - Smooth/standard: 0.001 for clean integer framerates
    const NES_TOLERANCE: f64 = 0.01;
    const STANDARD_TOLERANCE: f64 = 0.001;

    // Check for NES NTSC framerate and its multiples (within tolerance)
    // NES NTSC framerate: 39375000 / 655171 ≈ 60.098814
    for multiplier in 1..=10 {
        let target = NES_NTSC_FPS * multiplier as f64;
        if (fps - target).abs() < NES_TOLERANCE {
            let numerator = NES_NTSC_FPS_NUM * multiplier as u64;
            return format!("{}/{}", numerator, NES_NTSC_FPS_DEN);
        }
    }

    // Check for smooth framerate multiples (60, 120, 180, etc.)
    for multiplier in 1..=10 {
        let target = SMOOTH_FPS * multiplier as f64;
        if (fps - target).abs() < STANDARD_TOLERANCE {
            return format!("{}/1", 60 * multiplier);
        }
    }

    // Check for other common standard framerates
    if (fps - 30.0).abs() < STANDARD_TOLERANCE {
        return "30/1".to_string();
    }
    if (fps - 24.0).abs() < STANDARD_TOLERANCE {
        return "24/1".to_string();
    }
    if (fps - 59.94).abs() < NES_TOLERANCE {
        return "60000/1001".to_string(); // NTSC video standard
    }
    if (fps - 29.97).abs() < NES_TOLERANCE {
        return "30000/1001".to_string(); // NTSC video standard
    }
    if (fps - 23.976).abs() < NES_TOLERANCE {
        return "24000/1001".to_string(); // Film standard
    }

    // For other framerates, use a high-precision rational approximation
    // by multiplying by 1000 and rounding to get integer numerator
    let numerator = (fps * 1000.0).round() as u64;
    format!("{}/1000", numerator)
}

/// Encode collected frames to video.
pub fn encode_frames(
    frames: &[Vec<RgbColor>],
    format: VideoFormat,
    output_path: &Path,
    width: u32,
    height: u32,
    fps: f64,
) -> Result<u64, VideoError> {
    let mut encoder = create_encoder(format, output_path, width, height, fps)?;

    for frame in frames {
        encoder.write_frame(frame)?;
    }

    encoder.finish()?;
    Ok(encoder.frames_written())
}

/// Check if FFmpeg is available for MP4 encoding.
pub fn is_ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// =============================================================================
// Streaming Video Encoder
// =============================================================================

/// A streaming video encoder that handles scaling via FFmpeg.
///
/// This encoder is designed for use during emulation - frames are written
/// immediately as they are generated, without buffering all frames in memory.
///
/// Scaling is handled natively by FFmpeg using nearest-neighbor interpolation,
/// which is efficient and produces sharp pixel edges.
pub struct StreamingVideoEncoder {
    encoder: Box<dyn VideoEncoder>,
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    fps_config: FpsConfig,
}

impl StreamingVideoEncoder {
    /// Create a new streaming encoder with explicit FPS configuration.
    ///
    /// This is the preferred constructor when using the new FPS multiplier
    /// system.
    pub fn with_fps_config(
        format: VideoFormat,
        output_path: &Path,
        src_width: u32,
        src_height: u32,
        resolution: &VideoResolution,
        fps_config: FpsConfig,
    ) -> Result<Self, VideoError> {
        let (dst_width, dst_height) = resolution.dimensions(src_width, src_height);
        let fps = fps_config.output_fps();

        let encoder: Box<dyn VideoEncoder> = match format {
            VideoFormat::Mp4 => {
                if dst_width != src_width || dst_height != src_height {
                    // Use FFmpeg native scaling
                    Box::new(FfmpegMp4Encoder::new(
                        output_path,
                        src_width,
                        src_height,
                        fps,
                        Some((dst_width, dst_height)),
                    )?)
                } else {
                    Box::new(FfmpegMp4Encoder::new(
                        output_path,
                        src_width,
                        src_height,
                        fps,
                        None,
                    )?)
                }
            }
            _ => {
                // For non-MP4 formats, no scaling (use native resolution)
                if dst_width != src_width || dst_height != src_height {
                    eprintln!(
                        "Warning: Scaling only supported for MP4 format. Using native resolution."
                    );
                }
                create_encoder(format, output_path, src_width, src_height, fps)?
            }
        };

        Ok(Self {
            encoder,
            src_width,
            src_height,
            dst_width,
            dst_height,
            fps_config,
        })
    }

    /// Write a single frame.
    pub fn write_frame(&mut self, frame: &[RgbColor]) -> Result<(), VideoError> {
        self.encoder.write_frame(frame)
    }

    /// Finish encoding and finalize the output file.
    pub fn finish(&mut self) -> Result<(), VideoError> { self.encoder.finish() }

    /// Get the number of frames written so far.
    pub fn frames_written(&self) -> u64 { self.encoder.frames_written() }

    /// Get the source dimensions.
    pub fn source_dimensions(&self) -> (u32, u32) { (self.src_width, self.src_height) }

    /// Get the output dimensions (after scaling).
    pub fn output_dimensions(&self) -> (u32, u32) { (self.dst_width, self.dst_height) }

    /// Check if scaling is enabled.
    pub fn is_scaling(&self) -> bool {
        self.dst_width != self.src_width || self.dst_height != self.src_height
    }

    /// Get the FPS configuration.
    pub fn fps_config(&self) -> &FpsConfig { &self.fps_config }

    /// Check if mid-frame captures are needed.
    pub fn needs_mid_frame_capture(&self) -> bool { self.fps_config.needs_mid_frame_capture() }

    /// Get the number of captures per PPU frame.
    pub fn captures_per_frame(&self) -> u32 { self.fps_config.captures_per_frame() }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_resolution_parse() {
        assert_eq!(
            VideoResolution::parse("native").unwrap(),
            VideoResolution::Native
        );
        assert_eq!(
            VideoResolution::parse("1x").unwrap(),
            VideoResolution::Native
        );
        assert_eq!(
            VideoResolution::parse("2x").unwrap(),
            VideoResolution::IntegerScale(2)
        );
        assert_eq!(
            VideoResolution::parse("4x").unwrap(),
            VideoResolution::IntegerScale(4)
        );
        assert_eq!(
            VideoResolution::parse("720p").unwrap(),
            VideoResolution::Hd720
        );
        assert_eq!(
            VideoResolution::parse("1080p").unwrap(),
            VideoResolution::Hd1080
        );
        assert_eq!(
            VideoResolution::parse("4k").unwrap(),
            VideoResolution::Uhd4k
        );
        assert_eq!(
            VideoResolution::parse("1920x1080").unwrap(),
            VideoResolution::Custom(1920, 1080)
        );
    }

    #[test]
    fn test_video_resolution_dimensions() {
        // Native
        assert_eq!(VideoResolution::Native.dimensions(256, 240), (256, 240));

        // Integer scale
        assert_eq!(
            VideoResolution::IntegerScale(2).dimensions(256, 240),
            (512, 480)
        );
        assert_eq!(
            VideoResolution::IntegerScale(4).dimensions(256, 240),
            (1024, 960)
        );

        // 1080p should fit within bounds
        let (w, h) = VideoResolution::Hd1080.dimensions(256, 240);
        assert!(w <= 1920);
        assert!(h <= 1080);
    }

    #[test]
    fn test_video_error_display() {
        let err = VideoError::FfmpegNotFound;
        assert!(err.to_string().contains("FFmpeg not found"));

        let err = VideoError::InvalidDimensions {
            expected: (256, 240),
            got: (128, 120),
        };
        assert!(err.to_string().contains("256x240"));
        assert!(err.to_string().contains("128x120"));
    }

    #[test]
    fn test_png_encoder_frame_path() {
        let encoder = PngSequenceEncoder::new(Path::new("/tmp/test/frames"), 256, 240).unwrap();
        let path = encoder.frame_path(0);
        assert!(path.to_string_lossy().contains("frames_000000.png"));

        let path = encoder.frame_path(42);
        assert!(path.to_string_lossy().contains("frames_000042.png"));
    }

    #[test]
    fn test_ppm_encoder_frame_path() {
        let encoder = PpmSequenceEncoder::new(Path::new("/tmp/test/output"), 256, 240).unwrap();
        let path = encoder.frame_path(123);
        assert!(path.to_string_lossy().contains("output_000123.ppm"));
    }

    #[test]
    fn test_ffmpeg_availability_check() { let _available = is_ffmpeg_available(); }

    #[test]
    fn test_invalid_frame_dimensions() {
        let mut encoder =
            PpmSequenceEncoder::new(Path::new("/tmp/test_invalid"), 256, 240).unwrap();

        let bad_frame: Vec<RgbColor> = vec![RgbColor::new(0, 0, 0); 100];
        let result = encoder.write_frame(&bad_frame);

        assert!(result.is_err());
        if let Err(VideoError::InvalidDimensions {
            expected, ..
        }) = result
        {
            assert_eq!(expected, (256, 240));
        } else {
            panic!("Expected InvalidDimensions error");
        }
    }

    #[test]
    fn test_fps_to_rational() {
        // NES NTSC framerate (39375000 / 655171)
        let nes_ntsc = 39375000.0 / 655171.0;
        assert_eq!(fps_to_rational(nes_ntsc), "39375000/655171");

        // Standard framerates
        assert_eq!(fps_to_rational(60.0), "60/1");
        assert_eq!(fps_to_rational(30.0), "30/1");
        assert_eq!(fps_to_rational(24.0), "24/1");

        // NTSC video standards
        assert_eq!(fps_to_rational(59.94), "60000/1001");
        assert_eq!(fps_to_rational(29.97), "30000/1001");
        assert_eq!(fps_to_rational(23.976), "24000/1001");

        // Custom framerate (fallback to x/1000)
        assert_eq!(fps_to_rational(50.0), "50000/1000");

        // NES NTSC multiples (accurate mode)
        let nes_ntsc_2x = nes_ntsc * 2.0;
        assert_eq!(fps_to_rational(nes_ntsc_2x), "78750000/655171");

        // Smooth mode multiples
        assert_eq!(fps_to_rational(120.0), "120/1");
        assert_eq!(fps_to_rational(180.0), "180/1");
    }

    #[test]
    fn test_fps_config_parse_multipliers() {
        // Parse multipliers
        let config = FpsConfig::parse("1x", VideoExportMode::Accurate).unwrap();
        assert_eq!(config.multiplier, 1);
        assert_eq!(config.mode, VideoExportMode::Accurate);

        let config = FpsConfig::parse("2x", VideoExportMode::Accurate).unwrap();
        assert_eq!(config.multiplier, 2);

        let config = FpsConfig::parse("3x", VideoExportMode::Smooth).unwrap();
        assert_eq!(config.multiplier, 3);
        assert_eq!(config.mode, VideoExportMode::Smooth);
    }

    #[test]
    fn test_fps_config_parse_fixed_values() {
        // Parse fixed FPS values - should convert to multipliers
        let config = FpsConfig::parse("60.0", VideoExportMode::Smooth).unwrap();
        assert_eq!(config.multiplier, 1); // 60/60 = 1x

        let config = FpsConfig::parse("120", VideoExportMode::Smooth).unwrap();
        assert_eq!(config.multiplier, 2); // 120/60 = 2x

        let config = FpsConfig::parse("60.0988", VideoExportMode::Accurate).unwrap();
        assert_eq!(config.multiplier, 1); // ~60.0988/60.0988 = 1x

        let config = FpsConfig::parse("120.2", VideoExportMode::Accurate).unwrap();
        assert_eq!(config.multiplier, 2); // ~120.2/60.0988 ≈ 2x
    }

    #[test]
    fn test_fps_config_output_fps() {
        // Accurate mode at 1x
        let config = FpsConfig::parse("1x", VideoExportMode::Accurate).unwrap();
        assert!((config.output_fps() - NES_NTSC_FPS).abs() < 0.001);

        // Smooth mode at 1x
        let config = FpsConfig::parse("1x", VideoExportMode::Smooth).unwrap();
        assert!((config.output_fps() - 60.0).abs() < 0.001);

        // Accurate mode at 2x
        let config = FpsConfig::parse("2x", VideoExportMode::Accurate).unwrap();
        assert!((config.output_fps() - NES_NTSC_FPS * 2.0).abs() < 0.001);

        // Smooth mode at 2x
        let config = FpsConfig::parse("2x", VideoExportMode::Smooth).unwrap();
        assert!((config.output_fps() - 120.0).abs() < 0.001);
    }

    #[test]
    fn test_fps_config_output_rational() {
        // Accurate mode at 1x - exact NES framerate
        let config = FpsConfig::parse("1x", VideoExportMode::Accurate).unwrap();
        assert_eq!(config.output_fps_rational(), "39375000/655171");

        // Smooth mode at 1x - clean 60fps
        let config = FpsConfig::parse("1x", VideoExportMode::Smooth).unwrap();
        assert_eq!(config.output_fps_rational(), "60/1");

        // Accurate mode at 2x
        let config = FpsConfig::parse("2x", VideoExportMode::Accurate).unwrap();
        assert_eq!(config.output_fps_rational(), "78750000/655171");

        // Smooth mode at 2x
        let config = FpsConfig::parse("2x", VideoExportMode::Smooth).unwrap();
        assert_eq!(config.output_fps_rational(), "120/1");
    }

    #[test]
    fn test_fps_config_parse_errors() {
        // Invalid multiplier
        assert!(FpsConfig::parse("0x", VideoExportMode::Accurate).is_err());
        assert!(FpsConfig::parse("-1x", VideoExportMode::Accurate).is_err());

        // Invalid value
        assert!(FpsConfig::parse("abc", VideoExportMode::Accurate).is_err());
        assert!(FpsConfig::parse("", VideoExportMode::Accurate).is_err());

        // Negative FPS
        assert!(FpsConfig::parse("-60", VideoExportMode::Accurate).is_err());
    }

    #[test]
    fn test_fps_config_captures_per_frame() {
        let config = FpsConfig::parse("1x", VideoExportMode::Accurate).unwrap();
        assert_eq!(config.captures_per_frame(), 1);
        assert!(!config.needs_mid_frame_capture());

        let config = FpsConfig::parse("2x", VideoExportMode::Accurate).unwrap();
        assert_eq!(config.captures_per_frame(), 2);
        assert!(config.needs_mid_frame_capture());

        let config = FpsConfig::parse("3x", VideoExportMode::Smooth).unwrap();
        assert_eq!(config.captures_per_frame(), 3);
        assert!(config.needs_mid_frame_capture());
    }
}
