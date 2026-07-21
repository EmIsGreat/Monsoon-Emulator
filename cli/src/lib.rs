#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(unused_must_use)]
#![deny(unsafe_op_in_unsafe_fn)]
#![forbid(unsafe_code)]

pub mod cli;

use monsoon_core::declare_renderers;
use monsoon_core::emulation::screen_renderer::{
    NoneRenderer, RendererRegistration, ScreenRenderer,
};
use monsoon_default_renderers::LookupPaletteRenderer;

declare_renderers!(LookupPaletteRenderer, NoneRenderer);
