# monsoon-core

Core NES emulation library — cycle-accurate MOS 6502 CPU, 2C02 PPU, memory subsystem, ROM parsing, and save states.

This is the primary crate for anyone wanting to embed NES emulation in their own project. It is part of the [Monsoon Emulator](https://github.com/EmIsGreat/Monsoon-Emulator) workspace.

## Public API Modules

- **`emulation::nes`** — The top-level `Nes` struct that orchestrates all emulation. Provides methods to load ROMs, step individual cycles or full frames, save/load state, read the pixel buffer, and access CPU/PPU debug information.
- **`emulation::rom`** — ROM file parsing for iNES, NES 2.0, and archaic iNES formats. Includes `RomFile` for loading ROMs from bytes and `RomBuilder` for programmatic construction.
- **`emulation::savestate`** — Serializable emulator state snapshots. Save states can be encoded in binary (postcard) or JSON format.
- **`emulation::screen_renderer`** — The `ScreenRenderer` trait for implementing custom pixel renderers, plus `NoneRenderer` (a no-op fallback) and the `declare_renderers!` macro for registering renderers.
- **`emulation::palette_util`** — NES color palette types (`RgbColor`, `RgbPalette`) and `.pal` file parsing.
- **`emulation::ppu_util`** — PPU constants (output dimensions, tile counts) and debug data types (`EmulatorFetchable`, `TileData`, `PaletteData`, `NametableData`).
- **`util`** — Serialization helpers (`ToBytes`) and hashing utilities (`Hashable`).

Internal implementation modules (`cpu`, `ppu`, `mem`, `opcode`) are `pub(crate)` and not accessible to downstream consumers.

## Quick Start

```rust,no_run
use monsoon_core::emulation::nes::Nes;
use monsoon_core::emulation::rom::RomFile;

fn main() {
    let mut nes = Nes::default();

    // Load a ROM from raw bytes
    let rom_data = std::fs::read("game.nes").unwrap();
    let rom = RomFile::load(&rom_data, Some("game.nes".to_string())).unwrap();
    nes.load_rom(&rom);

    // Power on and run a single frame
    nes.power();
    nes.step_frame().expect("emulation error");

    // Get the rendered frame as a buffer of palette indices
    let pixels = nes.get_pixel_buffer();
    println!("Frame rendered: {} pixels", pixels.len());
}
```

## Pixel Buffer Format

`Nes::get_pixel_buffer()` returns a `Vec<u16>` of palette indices, **not** RGB values. Each 16-bit entry encodes:

- **Bits 0–5**: NES color index (0–63)
- **Bits 6–8**: Emphasis bits from the PPU mask register

Use a `ScreenRenderer` implementation (e.g., `LookupPaletteRenderer` from the `monsoon-default-renderers` crate) to convert these indices to RGB colors.

## Save States

```rust,no_run
use monsoon_core::emulation::nes::Nes;
use monsoon_core::emulation::savestate::try_load_state_from_bytes;
use monsoon_core::util::ToBytes;

let mut nes = Nes::default();

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

## License

This project is licensed under the [Apache-2.0 License](../LICENSE).
