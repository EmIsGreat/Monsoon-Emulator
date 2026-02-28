pub mod cli;

use monsoon_core::declare_renderers;
use monsoon_core::emulation::screen_renderer::{NoneRenderer, RendererRegistration, ScreenRenderer};
use monsoon_default_renderers::LookupPaletteRenderer;

declare_renderers!(LookupPaletteRenderer, NoneRenderer);
