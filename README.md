# Monsoon Emulator

A cycle-accurate NES (Nintendo Entertainment System) emulator written in Rust. Monsoon aims for maximum hardware accuracy on hard timing limits while allowing customizability for soft limits and hardware variables that cannot be perfectly emulated.

## Project Structure

Monsoon is organized as a Cargo workspace with four crates:

| Crate                    | Package Name                | Description                                                                   |
|--------------------------|-----------------------------|-------------------------------------------------------------------------------|
| [`core`](./core)         | `monsoon-core`              | Core emulation library — CPU, PPU, memory, ROM parsing, save states           |
| [`renderer`](./renderer) | `monsoon-default-renderers` | Default screen renderer implementations (lookup table-based palette renderer) |
| [`cli`](./cli)           | `monsoon-cli`               | Headless command-line interface for scripted/batch emulation                  |
| [`frontend`](./frontend) | `monsoon-frontend`          | GUI application built with [egui](https://github.com/emilk/egui)              |

### `monsoon-core`

The core emulation library. This is the primary crate for anyone wanting to embed NES emulation in their own project.

**Public API modules:**

- **`emulation::nes`** — The top-level [`Nes`] struct that orchestrates all emulation. Provides methods to load ROMs, step individual cycles or full frames, save/load state, read the pixel buffer, and access CPU/PPU debug information.
- **`emulation::rom`** — ROM file parsing for iNES, NES 2.0, and archaic iNES formats. Includes [`RomFile`] for loading ROMs from bytes and [`RomBuilder`] for programmatic construction.
- **`emulation::savestate`** — Serializable emulator state snapshots. Save states can be encoded in binary (postcard) or JSON format.
- **`emulation::screen_renderer`** — The [`ScreenRenderer`] trait for implementing custom pixel renderers, plus [`NoneRenderer`] (a no-op fallback) and the [`declare_renderers!`] macro for registering renderers.
- **`emulation::palette_util`** — NES color palette types ([`RgbColor`], [`RgbPalette`]) and `.pal` file parsing.
- **`emulation::ppu_util`** — PPU constants (output dimensions, tile counts) and debug data types (`EmulatorFetchable`, `TileData`, `PaletteData`, `NametableData`).
- **`util`** — Serialization helpers ([`ToBytes`]) and hashing utilities ([`Hashable`]).

Internal implementation modules (`cpu`, `ppu`, `mem`, `opcode`) are `pub(crate)` and not accessible to downstream consumers.

### `monsoon-default-renderers`

Provides [`LookupPaletteRenderer`], a fast O(1) lookup table-based renderer that converts the NES PPU's 9-bit palette indices into RGB colors. This is the default renderer used by both the frontend and CLI.

### `monsoon-cli`

A headless command-line interface for running NES ROMs without a GUI. Supports:

- Running ROMs for a specified number of frames or cycles
- Capturing screenshots and frame sequences
- Loading and saving state files
- Memory initialization from files
- Pluggable renderers via `--renderer` flag
- Video output to image files

See [`docs/CLI_INTERFACE.md`](./docs/CLI_INTERFACE.md) for the full CLI reference.

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

# List available renderers
cargo run -p monsoon-cli --bin cli -- --list-renderers
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
monsoon-core = { version = "0.1.0" }

# Optional: include the default renderer
monsoon-default-renderers = { version = "0.1.0" }
```

### Basic Usage

```rust,no_run
use monsoon_core::emulation::nes::Nes;
use monsoon_core::emulation::rom::RomFile;

fn main() {
    // Create the emulator
    let mut nes = Nes::default();

    // Load a ROM
    let rom_data = std::fs::read("game.nes").unwrap();
    let rom = RomFile::load(&rom_data, Some("game.nes".to_string())).unwrap();
    nes.load_rom(&rom);

    // Power on
    nes.power();

    // Run one frame
    nes.step_frame().expect("emulation error");

    // Read the pixel buffer (256x240 palette indices)
    let pixels = nes.get_pixel_buffer();
    println!("Frame rendered: {} pixels", pixels.len());
}
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

## Workspace Dependency Management

Internal crate dependencies are managed through [Cargo workspace dependencies](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-dependencies-table). The root `Cargo.toml` defines shared dependencies with both `version` and `path`:

```toml
[workspace.dependencies]
monsoon-core = { version = "0.1.0", path = "core" }
monsoon-default-renderers = { version = "0.1.0", path = "renderer" }
```

Member crates reference these with `workspace = true`:

```toml
[dependencies]
monsoon-core = { workspace = true }
```

This means:
- **Within the workspace**: Cargo always uses the local path (the `path` field).
- **As an external dependency**: When published to crates.io, dependents resolve by `version` from the registry.

## Testing

```bash
# Run all tests (core has ~335 tests, takes ~5 minutes)
cargo test -p monsoon-core

# Run a specific test
cargo test -p monsoon-core -- tests::nes::nestest

# Run doc tests only
cargo test -p monsoon-core --doc
```

## Build Profiles

| Profile | Command | Description |
|---------|---------|-------------|
| `dev` | `cargo build` | Debug build, no optimizations |
| `release` | `cargo build --release` | Optimized, stripped, abort on panic |
| `full_release` | `cargo build --profile full_release` | Release + LTO + single codegen unit |
| `max_performance` | `cargo build --profile max_performance` | Full release + native CPU targeting |
| `profiling` | `cargo build --profile profiling` | Release with debug symbols for profiling |

## License

This project is licensed under the [MIT License](LICENSE).
