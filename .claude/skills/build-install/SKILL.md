---
name: build-install
description: Build the Metaloop VST3/CLAP plugin and install to system VST3 folder. Use after code changes to test the plugin in a DAW.
---

# Build and Install Metaloop

Build the plugin bundle and install the VST3 to the system plugin directory.

## Steps

1. Run `bash bmv.sh` from the project root `/Users/rtu/dev/Rust/projects/metaloop`
2. This will:
   - Bundle the VST3 and CLAP plugins via `cargo xtask bundle metaloop --release`
   - Remove the old VST3 from `/Library/Audio/Plug-Ins/VST3/`
   - Move the new VST3 bundle into place
3. Report whether the build succeeded or failed, including any compiler warnings
4. If the build fails, analyze the errors and suggest fixes
