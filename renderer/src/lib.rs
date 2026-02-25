mod pixel_renderer;
// Re-export the renderer implementations
pub use pixel_renderer::LookupPaletteRenderer;

/// Returns all renderer registrations from this crate.
///
/// Used on WASM where the `inventory` crate's linker-based collection
/// doesn't work. On native, renderers self-register via `inventory::submit!`.
#[cfg(target_arch = "wasm32")]
pub fn get_renderer_registrations() -> Vec<ensemble_core::emulation::screen_renderer::RendererRegistration> {
    use ensemble_core::emulation::screen_renderer::RendererRegistration;
    vec![
        RendererRegistration {
            name: "LookupPaletteRenderer",
            factory: || Box::new(pixel_renderer::LookupPaletteRenderer::new()),
        },
        RendererRegistration {
            name: "LookupPaletteRenderer2",
            factory: || Box::new(pixel_renderer2::LookupPaletteRenderer2::new()),
        },
    ]
}
