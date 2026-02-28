//! Screen rendering abstraction.
//!
//! This module defines the [`ScreenRenderer`] trait for converting the NES PPU's
//! palette index buffer into RGB pixel data. It also provides [`NoneRenderer`]
//! (a no-op implementation), [`RendererRegistration`] for registering custom
//! renderers, and the [`create_renderer`] factory function.
//!
//! For a full-featured renderer, see `LookupPaletteRenderer` in the
//! `monsoon-default-renderers` crate.
//!
//! # Implementing a Custom Renderer
//!
//! ```rust,no_run
//! use std::fmt::{Debug, Formatter};
//!
//! use monsoon_core::emulation::palette_util::{RgbColor, RgbPalette};
//! use monsoon_core::emulation::screen_renderer::ScreenRenderer;
//!
//! struct MyRenderer {/* ... */}
//!
//! impl Debug for MyRenderer {
//!     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { f.write_str("MyRenderer") }
//! }
//!
//! impl ScreenRenderer for MyRenderer {
//!     fn buffer_to_image(&mut self, buffer: &[u16]) -> &[RgbColor] {
//!         // Convert palette indices to RGB colors
//!         todo!()
//!     }
//!
//!     fn set_palette(&mut self, palette: RgbPalette) { /* ... */
//!     }
//!
//!     fn get_width(&self) -> usize { 256 }
//!
//!     fn get_height(&self) -> usize { 240 }
//!
//!     fn get_id(&self) -> &'static str { "my_renderer" }
//!
//!     fn get_display_name(&self) -> &'static str { "My Custom Renderer" }
//! }
//! ```

use std::fmt::{Debug, Formatter};

use crate::emulation::palette_util::{RgbColor, RgbPalette};

/// Trait for rendering NES palette indices to RGB colors.
///
/// The NES PPU outputs a buffer of 16-bit palette indices (see
/// [`Nes::get_pixel_buffer`](crate::emulation::nes::Nes::get_pixel_buffer)).
/// Implementations of this trait convert those indices into [`RgbColor`] values
/// that can be displayed on screen.
///
/// Implementations must also be [`Debug`] to support logging and diagnostics.
pub trait ScreenRenderer: Debug {
    /// Converts a buffer of palette indices to RGB colors.
    ///
    /// The input `buffer` contains 16-bit values where bits 0-5 are the NES
    /// color index and bits 6-8 are emphasis bits. The returned slice must
    /// have the same length as `buffer`.
    ///
    /// The returned reference is valid until the next call to this method.
    fn buffer_to_image(&mut self, buffer: &[u16]) -> &[RgbColor];

    /// Updates the palette used for color conversion.
    ///
    /// Called when the user loads a new `.pal` palette file.
    fn set_palette(&mut self, palette: RgbPalette);

    /// Returns the output image width in pixels (typically 256).
    fn get_width(&self) -> usize;

    /// Returns the output image height in pixels (typically 240).
    fn get_height(&self) -> usize;

    /// Returns a unique string identifier for this renderer (e.g., `"PaletteLookup"`).
    fn get_id(&self) -> &'static str;

    /// Returns a human-readable display name for this renderer.
    fn get_display_name(&self) -> &'static str;
}

type RendererFactory = fn() -> Box<dyn ScreenRenderer>;

/// A registration entry for a [`ScreenRenderer`] implementation.
///
/// Used by the renderer discovery system to enumerate available renderers
/// at runtime. Each registration associates a unique key and display name
/// with a factory function that creates renderer instances.
#[derive(Copy, Clone, Eq)]
pub struct RendererRegistration {
    /// Unique identifier matching [`ScreenRenderer::get_id()`].
    pub key: &'static str,
    /// Human-readable name matching [`ScreenRenderer::get_display_name()`].
    pub display_name: &'static str,
    /// Factory function that creates a new boxed renderer instance.
    pub factory: RendererFactory,
}

impl PartialEq for RendererRegistration {
    fn eq(&self, other: &Self) -> bool { self.key == other.key }
}

/// Declares which [`ScreenRenderer`](crate::emulation::screen_renderer::ScreenRenderer)
/// implementations are available in a crate.
///
/// This macro generates a `get_all_renderers()` function that returns a
/// `Vec<RendererRegistration>` for each listed type.
///
/// # Example
///
/// ```rust,ignore
/// use monsoon_core::declare_renderers;
/// use monsoon_core::emulation::screen_renderer::{RendererRegistration, ScreenRenderer};
///
/// declare_renderers!(MyRendererA, MyRendererB);
/// ```
#[macro_export]
macro_rules! declare_renderers {
    ($($ty:ty),* $(,)?) => {
        pub fn get_all_renderers() -> Vec<RendererRegistration> {
            vec![
                $(
                    RendererRegistration {
                        key: <$ty>::new().get_id(),
                        display_name: <$ty>::new().get_display_name(),
                        factory: || Box::new(<$ty>::new()),
                    }
                ),*
            ]
        }
    };
}

/// Creates a boxed [`ScreenRenderer`] by name from a list of registrations.
///
/// If `name` is `Some` and matches a registration's key, that renderer is
/// instantiated. Otherwise, a [`NoneRenderer`] is returned as a fallback.
///
/// # Arguments
///
/// * `name` — The renderer key to look up, or `None` for the fallback.
/// * `renderers` — Available renderer registrations (typically from
///   [`declare_renderers!`](crate::declare_renderers)).
pub fn create_renderer(
    name: Option<&str>,
    renderers: Vec<RendererRegistration>,
) -> Box<dyn ScreenRenderer> {
    if let Some(name) = name {
        let renderer = renderers.iter().find(|r| r.key == name);

        if let Some(renderer) = renderer {
            (renderer.factory)()
        } else {
            Box::new(NoneRenderer::new())
        }
    } else {
        Box::new(NoneRenderer::new())
    }
}

/// A no-op renderer that produces a single default-colored pixel.
///
/// This is the fallback renderer used when no other renderer is available
/// or when rendering is not needed (e.g., headless mode without video output).
pub struct NoneRenderer {
    image: [RgbColor; 1],
}

impl Default for NoneRenderer {
    fn default() -> Self { Self::new() }
}

impl NoneRenderer {
    /// Creates a new `NoneRenderer`.
    pub fn new() -> Self {
        NoneRenderer {
            image: [RgbColor::default()],
        }
    }
}

impl Debug for NoneRenderer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get_display_name())
    }
}

impl ScreenRenderer for NoneRenderer {
    fn buffer_to_image(&mut self, _: &[u16]) -> &[RgbColor] { &self.image }

    fn set_palette(&mut self, _: RgbPalette) {}

    fn get_width(&self) -> usize { 1 }

    fn get_height(&self) -> usize { 1 }

    fn get_id(&self) -> &'static str { "none" }

    fn get_display_name(&self) -> &'static str { "None" }
}
