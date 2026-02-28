# monsoon-cli

Headless command-line interface for the Monsoon NES emulator.

A CLI application for running NES ROMs without a GUI, useful for scripted/batch emulation, automated testing, and screenshot capture. It is part of the [Monsoon Emulator](https://github.com/EmIsGreat/Monsoon-Emulator) workspace.

## Features

- Running ROMs for a specified number of frames or cycles
- Capturing screenshots and frame sequences
- Loading and saving state files
- Memory initialization from files
- Pluggable renderers via `--renderer` flag
- Video output to image files

## Quick Start

```bash
# Run a ROM for 60 frames and capture a screenshot
cargo run -p monsoon-cli --bin cli -- \
  --rom path/to/game.nes \
  --frames 60 \
  --screenshot output.png

# List available renderers
cargo run -p monsoon-cli --bin cli -- --list-renderers
```

See [`docs/CLI_INTERFACE.md`](../docs/CLI_INTERFACE.md) for the full CLI reference.

## License

This project is licensed under the [Apache-2.0 License](../LICENSE).
