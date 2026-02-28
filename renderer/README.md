# monsoon-default-renderers

Default screen renderer implementations for the Monsoon NES emulator.

This crate provides `LookupPaletteRenderer`, a fast O(1) lookup table-based renderer that converts the NES PPU's 9-bit palette indices into RGB colors. It is the default renderer used by both the frontend and CLI applications. It is part of the [Monsoon Emulator](https://github.com/EmIsGreat/Monsoon-Emulator) workspace.

## Usage

Add `monsoon-default-renderers` alongside `monsoon-core`:

```toml
[dependencies]
monsoon-core = { version = "0.1.0" }
monsoon-default-renderers = { version = "0.1.0" }
```

```rust,no_run
use monsoon_core::emulation::screen_renderer::ScreenRenderer;
use monsoon_default_renderers::LookupPaletteRenderer;

let mut renderer = LookupPaletteRenderer::new();

// pixel_buffer is a &[u16] from Nes::get_pixel_buffer()
# let pixel_buffer: &[u16] = &[];
let rgb_pixels = renderer.buffer_to_image(pixel_buffer);
// rgb_pixels is a &[RgbColor] — each with .r, .g, .b fields (u8)
```

## License

This project is licensed under the [Apache-2.0 License](../LICENSE).
