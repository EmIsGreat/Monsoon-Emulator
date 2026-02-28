//! Default screen renderer implementations for the Monsoon NES emulator.
//!
//! This crate provides [`LookupPaletteRenderer`], a fast O(1) lookup table-based
//! renderer that converts NES palette indices to RGB colors. It is the default
//! renderer used by both the frontend and CLI applications.
//!
//! # Example
//!
//! ```rust,no_run
//! use monsoon_core::emulation::screen_renderer::ScreenRenderer;
//! use monsoon_default_renderers::LookupPaletteRenderer;
//!
//! let mut renderer = LookupPaletteRenderer::new();
//!
//! // pixel_buffer is a &[u16] from Nes::get_pixel_buffer()
//! # let pixel_buffer: &[u16] = &[];
//! let rgb_pixels = renderer.buffer_to_image(pixel_buffer);
//! ```

mod pixel_renderer;
// Re-export the renderer implementations
pub use pixel_renderer::LookupPaletteRenderer;
