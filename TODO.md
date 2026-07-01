# Metaloop TODO

Fresh pass over `src/` as of the merged codebase. The previous `TODO.md`
(see `git show pre-sync-backup:TODO.md`) is stale — its one item (splitting
`GrainLooper`) is repeated below since it's still true, but everything else
here is newly verified against the current code.

## P0 — known bug: 6 failing tests in `grain_looper.rs`

`cargo test --lib` currently fails 6 of the `grain_looper::tests` tests
(`test_grain_looper_loop`, `test_grain_looper_loop_offset`,
`test_grain_looper_tweak_loop`, `test_grain_looper_immediate_reverse`,
`test_grain_looper_short_to_long`, `test_grain_looper_change_tempo`) because
of a `debug_assert!` added in commit `1011aa1` ("Add assert that if dry is
zero we should have grains TODO fix this"):

```
Grain player is in an inconsistent state, dry_level: 0, num_playing_grains: 0
```

In `GrainLooper::tick` (`grain_looper.rs`), the assumption is that whenever
the dry signal is fully faded out (`dry_level == 0.0`), there must be at
least one grain playing to cover the gap. In practice the dry ramp can reach
zero on a sample where the grain player also has zero playing grains
(e.g. right as one grain finishes and before the next one's scheduled start,
or during loop-length/offset changes), so the assert fires. This needs an
actual fix to the scheduling/fade timing (so a grain is always active before
the dry fades out), not just relaxing or removing the assert.

## P1 — architecture

### `GrainLooper` mixes three concerns
`GrainLooper` (`grain_looper.rs`) currently does unit conversion (beats ↔
samples via `TimeConverter`), scheduling policy (interpreting `LoopEvent`s
from `LoopScheduler`, deciding when to start/stop grains and ramp the dry
signal), and grain lifecycle plumbing (constructing `Grain`s and forwarding
to `GrainPlayer`) all in one struct. Candidate split:
- `LoopController` — parameter/state surface (offset, grid, fade, reverse,
  speed, tempo) exposed to `lib.rs`.
- `GrainScheduler` — wraps `LoopScheduler`/`Scheduler<LoopEvent>` and turns
  beat-time events into grain start/stop decisions.
- `GrainPlayer` — stays as-is, purely buffer/grain-pool management.

### `Grain::is_finished()` relies on an implicit sentinel
In `grain.rs`, `is_finished()` is:
```rust
pub fn is_finished(&self) -> bool {
    return self.elapsed_sample_count == self.duration || self.duration == 0;
}
```
The `duration == 0` branch exists only so that the pool-placeholder grains
created in `GrainPlayer::new_with_length` (`Grain::new(0.0, 0, 0, false, 0.0)`)
read as "finished" before ever being ticked, so `GrainPlayer::start_grain`
can find/reuse them via the same `is_finished()` check used for real,
completed grains. This overloads one method with two different meanings
("never started" vs "played to completion") and makes `duration == 0` a
magic sentinel rather than an explicit state. Worth making pool-slot vs.
active/finished an explicit enum or `Option<Grain>` instead.

## P2 — dead / WIP code

### `src/PGHI/heap.rs` is WIP and not wired into the build
Added in commit `e423ebd` ("WIP heap sort"). It is a generic max-heap
(`build_max_heap`, `insert_new`, `extract_max`, etc.) with its tests living
inside a `fn main()` rather than a `#[test]` module, so they don't even run
under `cargo test`. There is no `mod PGHI;` (or similar) declaration anywhere
in `lib.rs` or any other module — confirmed via `grep -rn "PGHI"`, which only
matches the file's own path. It is entirely disconnected from the crate and
does not currently compile as part of the build. Either finish wiring it in
with real tests, or remove it until it's needed.

### `src/ui/ui.rs` looks like dead leftover
`src/ui/mod.rs` only declares `mod my_param_slider;` and
`pub mod waveform_display;` — there is no `mod ui;`, so `src/ui/ui.rs`
(a 3-line file that just re-declares `mod my_param_slider;` and
`pub use my_param_slider::MyParamSlider;`) is not part of the module tree at
all and isn't compiled. Looks like a stray duplicate left over from
reorganizing the `ui` module; safe to delete once confirmed unused.

## P3 — smaller cleanups (from `cargo clippy --lib`)

`cargo clippy --lib` currently reports 32 warnings. Most are trivial style
issues, but a few are worth calling out:
- `TimeConverter::sample_rate()` is reported as never used (dead getter) —
  either use it or remove it.
- Several `unneeded return` / `manual implementation of an assign operation`
  (e.g. `x = x + 1` instead of `x += 1`) across `grain.rs`, `delay_line.rs`.
- `println!("Setting offset to {}", ...)` debug print left in
  `ui/my_param_slider.rs::set_normalized_x` — should probably be removed or
  gated behind a debug flag before shipping.
- Stale comment in `lib.rs::editor()`: `// this is bad` above the
  `create_egui_editor` call, with no further explanation of what's bad about
  it — worth either fixing or turning into an actionable TODO.
- `GrainLooper::num_playing_grains()` is a private method with no callers
  outside tests-adjacent debug code — check whether it's still needed or was
  superseded by the `dry_level`/`num_playing_grains` assert in `tick()`.
</content>
