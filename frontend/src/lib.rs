#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(unused_must_use)]
#![deny(unsafe_op_in_unsafe_fn)]

use monsoon_core::declare_renderers;
use monsoon_core::emulation::screen_renderer::{
    NoneRenderer, RendererRegistration, ScreenRenderer,
};
use monsoon_default_renderers::LookupPaletteRenderer;

pub mod channel_emu;
pub mod frontend;
pub mod messages;

declare_renderers!(LookupPaletteRenderer, NoneRenderer);
