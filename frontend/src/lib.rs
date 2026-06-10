#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(unused_must_use)]

use monsoon_core::emulation::screen_renderer::ScreenRenderer;
use monsoon_core::emulation::screen_renderer::RendererRegistration;
use monsoon_core::declare_renderers;
use monsoon_core::emulation::screen_renderer::NoneRenderer;
use monsoon_default_renderers::LookupPaletteRenderer;
use crate::frontend::WgpuScreenRenderer;

pub mod channel_emu;
pub mod frontend;
pub mod messages;

declare_renderers!(LookupPaletteRenderer, WgpuScreenRenderer, NoneRenderer);
