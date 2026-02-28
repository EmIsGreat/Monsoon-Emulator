# monsoon-frontend

Native GUI frontend for the Monsoon NES emulator, built with [egui](https://github.com/emilk/egui).

A desktop (and web) application for playing NES games with a full graphical interface. It is part of the [Monsoon Emulator](https://github.com/EmIsGreat/Monsoon-Emulator) workspace.

## Features

- ROM loading via file dialog or command-line argument
- Quick save/load and autosave support
- Save state browser (browse, load, export saves)
- PPU debug views (pattern tables, nametables, palettes)
- Custom palette file loading
- Pluggable screen renderers
- WASM support (runs in web browsers with IndexedDB storage)

## Quick Start

```bash
# Run with default settings
cargo run

# Run with a ROM file
cargo run -- --rom path/to/game.nes
```

## Building for WASM

The frontend supports WebAssembly builds for running in a web browser:

```bash
cargo build -p monsoon-frontend --target wasm32-unknown-unknown
```

## License

This project is licensed under the [Apache-2.0 License](../LICENSE).
