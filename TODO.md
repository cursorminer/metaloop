# Metaloop TODO

Fresh pass over `src/` as of the merged codebase. The previous `TODO.md`
(see `git show pre-sync-backup:TODO.md`) is stale — its one item (splitting
`GrainLooper`) is repeated below since it's still true, but everything else
here is newly verified against the current code.

## P0 — FIXED: looper can go silent under rapid start/stop cycling

Fixed by rearchitecting the scheduling core. The root cause was structural:
three independently-clocked mechanisms (the clearable `Scheduler<LoopEvent>`
queue, fire-and-forget `Grain`s, and the fire-and-forget `dry_ramp`) had to
stay paired to uphold "dry silent ⟹ grains audible", but every `.clear()`
destroyed future intent while past intent kept executing.

Two changes made the failure mode impossible by construction:

1. `LoopScheduler` is now a phase state machine (`Idle → Armed → Looping →
   Stopping`) with no event queue. Each tick *derives* due actions from the
   phase, the grid math, and the beat time when the sounding grain actually
   runs out (`grain_end`). Nothing about the future is pre-committed, so
   start/stop/grid changes never clear or reschedule anything — the next
   tick reconciles against what is actually sounding (including
   self-healing: if a grain ran out with nothing to replace it, the next
   tick bridges with a legato grain). A stop followed by a re-click before
   the stop's boundary resumes the loop seamlessly.
2. The dry level is derived from grain liveness each tick
   (`num_playing_non_fading_out_grains()`) instead of being faded by
   scheduled `FadeOutDry`/`FadeInDry` events. A grain fading out with no
   successor pulls dry back in over the same fade window, so silence cannot
   persist. `test_grain_looper_never_goes_silent` asserts this as a
   property under rapid click/drag/release cycling on a DC input.

The generic `Scheduler<E>` (`scheduler.rs`) was deleted along with the
`FadeOutDry`/`FadeInDry`/`NextLoop` events and the (now unreachable)
"inconsistent state" assertion.

## P1 — architecture

### `GrainLooper` mixes three concerns
`GrainLooper` (`grain_looper.rs`) still does unit conversion (beats ↔
samples via `TimeConverter`), event interpretation (turning `LoopEvent`s
from `LoopScheduler` into grain starts/stops and the liveness-derived dry
ramp), and grain lifecycle plumbing (constructing `Grain`s and forwarding
to `GrainPlayer`) in one struct. The state-machine rewrite shrank it, but a
split into a parameter/state surface and an event-translation layer is
still worthwhile if it keeps growing.

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

## P3 — smaller cleanups (from `cargo clippy --lib`)

`cargo clippy --lib` currently reports 32 warnings. Most are trivial style
issues, but a few are worth calling out:
- `TimeConverter::sample_rate()` is reported as never used (dead getter) —
  either use it or remove it.
- Several `unneeded return` / `manual implementation of an assign operation`
  (e.g. `x = x + 1` instead of `x += 1`) across `grain.rs`, `delay_line.rs`.
- `GrainLooper::num_playing_grains()` is a private method with no callers
  (the assert that used it is gone) — remove it or use it.
- `MyParamSlider` calls `setter.set_parameter()` in its drag handling without
  wrapping it in `begin_set_parameter()`/`end_set_parameter()` — nih-plug logs
  a debug assertion on every drag. Harmless standalone, but hosts use the
  begin/end pair for automation recording and undo grouping.
- The standalone binary aborts at startup if the requested sample rate doesn't
  match the output device's native rate (nih-plug's CPAL backend asserts on
  oversized CoreAudio callbacks and can't unwind). nih-plug defaults to 48 kHz;
  on a 44.1 kHz device run it as:
  `cargo run --features standalone --bin metaloop_gui -- --sample-rate 44100`
</content>
