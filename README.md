# Metaloop

A VST3/CLAP audio plugin for live loop, stutter, and glitch effects — built in Rust.

Metaloop captures incoming audio into a buffer and lets you instantly grab, reverse, pitch-shift, and repeat loops on the fly. Everything is beat-synced to your DAW's tempo, so loops always land on the grid.

## Features

- **XY pad control** — a single 2D pad controls loop offset and length, optimized for live performance
- **Waveform display** — real-time visualization of the audio buffer as you interact with it
- **Minimal interface** — designed for live jams where every control needs to be instantly accessible
- **Beat-synced looping** — loop lengths snap to musical subdivisions from 1/64th notes up to full bars
- **Reverse & pitch-shift** — flip loops backwards or change playback speed for tape-style effects

## Formats

- **VST3**
- **CLAP**

## Building

Requires [Rust](https://rustup.rs/).

```shell
# Build release VST3/CLAP bundles
cargo xtask bundle metaloop --release

# Run tests
cargo test
```

On macOS, you can install the built VST3 to the system plugin folder:

```shell
./bmv.sh
```

## License

Licensed under the [GNU General Public License v3.0](LICENSE).
