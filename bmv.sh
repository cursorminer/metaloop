#!/bin/bash
cargo xtask bundle metaloop --release;
rm -r /Library/Audio/Plug-Ins/VST3/Metaloop.vst3;
mv /Users/rtu/dev/Rust/projects/metaloop/target/bundled/Metaloop.vst3 /Library/Audio/Plug-Ins/VST3/