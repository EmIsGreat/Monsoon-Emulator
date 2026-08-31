# Monsoon Emulator

Time spent since 09 February 2026:
&nbsp;&nbsp;[![wakatime](https://wakatime.com/badge/user/c71acca9-7c34-4e24-94f2-b52a92e4673e/project/eb026d79-1ad1-4546-93a5-241f7054b420.svg)](https://wakatime.com/badge/user/c71acca9-7c34-4e24-94f2-b52a92e4673e/project/eb026d79-1ad1-4546-93a5-241f7054b420) * ~
1.3

A cycle-accurate NES (Nintendo Entertainment System) emulator written in Rust. Monsoon aims for maximum hardware accuracy on hard timing limits while allowing customizability for soft limits and hardware variables that cannot be perfectly
emulated.

## Project Structure

Monsoon is organized as a Cargo workspace with multiple crates:

| Crate                    | Package Name                | Description                                                                   |
|--------------------------|-----------------------------|-------------------------------------------------------------------------------|
| [`core`](core)           | `monsoon-core`              | Core emulation library — CPU, PPU, memory, ROM parsing, save states           |
| [`renderer`](./renderer) | `monsoon-default-renderers` | Default screen renderer implementations (lookup table-based palette renderer) |
| [`cli`](./cli)           | `monsoon-cli`               | Headless command-line interface for scripted/batch emulation                  |
| [`frontend`](./frontend) | `monsoon-frontend`          | GUI application built with [egui](https://github.com/emilk/egui)              |
| [`macros`](./macros)     | `monsoon-macros`            | Declares Proc-Macros used for monsoon-core                                    |
| [`db`](./db)             | `monsoon-db`                | Provides a Rom file lookup to enhanced compatibility                          |

### `monsoon-core`

The core emulation library. This is the primary crate for anyone wanting to embed NES emulation in their own project.

### `monsoon-frontend`

A native desktop GUI built with [egui](https://github.com/emilk/egui) and [eframe](https://github.com/emilk/egui/tree/master/crates/eframe). Features include:

- ROM loading via file dialog or command-line argument
- Quick save/load and autosave support
- Save state browser (browse, load, export saves)
- PPU debug views (pattern tables, nametables, palettes)
- Custom palette file loading
- Pluggable screen renderers
- WASM support (runs in web browsers with IndexedDB storage)

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (nightly toolchain — configured automatically via `rust-toolchain.toml`)

### Running the GUI Frontend

```bash
# Run with default settings
cargo run

# Run with a ROM file
cargo run -- --rom path/to/game.nes
```

### Running the Headless CLI

```bash
# Run a ROM for 60 frames and capture a screenshot
cargo run -p monsoon-cli --bin cli -- \
  --rom path/to/game.nes \
  --frames 60 \
  --screenshot output.png
```

### Building

```bash
# Build the entire workspace
cargo build

# Build only the core library
cargo build -p monsoon-core

# Build a release build with optimizations
cargo build --profile release

# Build a fully optimized release (LTO + single codegen unit)
cargo build --profile full_release
```

## Using `monsoon-core` as a Library

Add `monsoon-core` to your project's dependencies:

```toml
[dependencies]
monsoon-core = { version = "0.2.10" }

# Optional: include the default renderer
monsoon-default-renderers = { version = "0.2.10" }
```

### Pixel Buffer Format

`Nes::get_pixel_buffer()` returns a `Vec<u16>` of palette indices, **not** RGB values. Each 16-bit entry encodes:

- **Bits 0-5**: NES color index (0-63)
- **Bits 6-8**: Emphasis bits from the PPU mask register

Use a [`ScreenRenderer`] implementation to convert to RGB:

```rust,no_run
use monsoon_core::emulation::screen_renderer::ScreenRenderer;
use monsoon_default_renderers::LookupPaletteRenderer;

let mut renderer = LookupPaletteRenderer::new();

// pixel_buffer from Nes::get_pixel_buffer()
# let pixel_buffer: &[u16] = &[];
let rgb_pixels = renderer.buffer_to_image(pixel_buffer);
// rgb_pixels is a &[RgbColor] — each with .r, .g, .b fields (u8)
```

### Save States

```rust,no_run
use monsoon_core::emulation::nes::Nes;
use monsoon_core::emulation::savestate::try_load_state_from_bytes;
use monsoon_core::util::ToBytes;

# let mut nes = Nes::default();
// Save state
if let Some(state) = nes.save_state() {
    // Binary format (compact)
    let bytes = state.to_bytes(None);
    std::fs::write("save.state", &bytes).unwrap();

    // JSON format (human-readable)
    let json_bytes = state.to_bytes(Some("json".to_string()));
    std::fs::write("save.json", &json_bytes).unwrap();
}

// Load state
let data = std::fs::read("save.state").unwrap();
if let Some(state) = try_load_state_from_bytes(&data) {
    nes.load_state(state);
}
```

### Custom Screen Renderer

Implement the `ScreenRenderer` trait to create your own renderer:

```rust,no_run
use std::fmt::{Debug, Formatter};
use monsoon_core::emulation::palette_util::{RgbColor, RgbPalette};
use monsoon_core::emulation::screen_renderer::ScreenRenderer;

struct MyRenderer {
    palette: RgbPalette,
    buffer: Vec<RgbColor>,
}

impl Debug for MyRenderer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("MyRenderer")
    }
}

impl ScreenRenderer for MyRenderer {
    fn buffer_to_image(&mut self, buffer: &[u16]) -> &[RgbColor] {
        self.buffer.clear();
        for &index in buffer {
            let color = index as usize & 0x3F;
            let emphasis = (index as usize >> 6) & 0x7;
            self.buffer.push(self.palette.colors[emphasis][color]);
        }
        &self.buffer
    }

    fn set_palette(&mut self, palette: RgbPalette) {
        self.palette = palette;
    }

    fn get_width(&self) -> usize { 256 }
    fn get_height(&self) -> usize { 240 }
    fn get_id(&self) -> &'static str { "my_renderer" }
    fn get_display_name(&self) -> &'static str { "My Custom Renderer" }
}
```

## Build Profiles

| Profile        | Command                              | Description                              |
|----------------|--------------------------------------|------------------------------------------|
| `dev`          | `cargo build`                        | Debug build, no optimizations            |
| `release`      | `cargo build --release`              | Optimized, stripped, abort on panic      |
| `full_release` | `cargo build --profile full_release` | Release + LTO + single codegen unit      |
| `native`       | `cargo build --profile native`       | Full release + native CPU targeting      |
| `profiling`    | `cargo build --profile profiling`    | Release with debug symbols for profiling |

## License

This project is licensed under the [Apache 2.0](LICENSE).
