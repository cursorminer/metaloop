# Metaloop

A VST3/CLAP audio plugin that creates loop/stutter effects with reversed and pitched loops. 
It is intended to provide a wide range of effects with an extremely minimal and live 
jam optimised interface.
Built with Rust using the nih-plug framework and egui for UI.

## Architecture

### Audio Signal Flow
1. Raw audio is continuously written into a **rolling delay buffer** (DelayLine)
2. As looping progresses, audio is copied to a **static buffer** for indefinite looping
3. **Grains** read from the delay buffer at calculated positions with fade envelopes
4. Grains are mixed with the dry signal using crossfades during loop start/stop
5. Everything is beat-synchronized via `LoopScheduler` for DAW tempo integration

### Module Dependency Graph
```
Metaloop (Plugin - lib.rs)
  GrainLooper       - High-level looper orchestrator
    GrainPlayer     - Manages up to 10 concurrent grains + dual delay buffers
      Grain         - Single faded audio grain (position, envelope, direction, speed)
      DelayLine<T>  - Generic circular buffer with linear interpolation
    LoopScheduler   - Beat-synchronized event scheduling
      Scheduler<E>  - Generic time-ordered event queue
  RampedValue       - Linear value ramping for fades
  StereoPair<T>     - Generic stereo container with arithmetic ops
  UI (egui)
    WaveformDisplay - Waveform visualization widget
    MyParamSlider   - 2D XY pad for loop offset + length
```

### Key Trait
- `AudioSampleOps` (stereo_pair.rs) - Trait alias for types supporting audio arithmetic (Copy + Default + Add + Sub + Mul + AddAssign). Implemented for all qualifying types.

## Build & Test

```shell
# Build (debug)
cargo build

# Build release VST3/CLAP bundle
cargo xtask bundle metaloop --release

# Run tests
cargo test

# Install VST3 to system (macOS) - uses bmv.sh
./bmv.sh

# Check for warnings
cargo check
```

## Project Conventions

- **Framework**: nih-plug (git dependency from https://github.com/robbert-vdh/nih-plug.git)
- **UI**: nih_plug_egui (egui integration for nih-plug)
- **Stereo only**: 2-channel input/output
- **Beat-synced**: Loop lengths quantized to musical subdivisions (SYNCED_RATES table in lib.rs)
- **Tests**: Most modules have `#[cfg(test)] mod tests` blocks. Use `test_utils::all_near()` for float comparisons.
- **Dead code suppression**: Many modules use `#[allow(dead_code)]` since public APIs may not be used internally but are part of the module interface.

## Known Issues / TODOs
- `src/PGHI/heap.rs` is orphaned (not declared as a module) - WIP heap sort experiment
- `countdown_trigger.rs` is declared but unused
- Output is delayed by one sample (TODO in lib.rs:273)
- `loop_scheduler.rs:68` - `set_fade_lead_in()` is a no-op
- `grain_looper.rs` has a TODO about setting fade time in seconds vs samples
- Debug assertion in grain_looper.rs about dry_level being zero with no grains

## File Layout
```
src/
  lib.rs                  - Plugin entry point, params, process(), editor()
  grain.rs                - Single grain with fade envelope
  grain_player.rs         - Multi-grain manager with dual delay buffers
  grain_looper.rs         - Top-level looper logic
  loop_scheduler.rs       - Beat-synced event scheduler
  scheduler.rs            - Generic event queue
  delay_line.rs           - Circular buffer
  ramped_value.rs         - Linear value ramping
  stereo_pair.rs          - Stereo pair type + AudioSampleOps trait
  countdown_trigger.rs    - Countdown utility (unused)
  test_utils.rs           - Float comparison helpers
  PGHI/heap.rs            - Max heap (orphaned, WIP)
  ui/
    mod.rs                - UI module re-exports
    ui.rs                 - (empty or minimal)
    waveform_display.rs   - Waveform visualization
    my_param_slider.rs    - 2D XY control pad
```
