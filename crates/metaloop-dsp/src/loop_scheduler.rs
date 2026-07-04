// Decides what the looper does at each tick according to the beat time.
//
// This is a phase state machine rather than an event queue: on every tick the
// due actions are derived from the current phase, the grid math and the beat
// time at which the currently sounding grain runs out (`grain_end`). Nothing
// about the future is pre-committed, so parameter changes (grid, start/stop)
// never need to clear or reschedule anything - the next tick simply
// reconciles against what is actually sounding. This removes the class of
// bugs where clearing a queue orphaned an in-flight fade or grain.
use arrayvec::ArrayVec;

/// Maximum number of events that can fire on a single tick.
/// In practice this is at most 2 (StopGrain + StartGrain), 8 gives headroom.
pub const MAX_EVENTS_PER_TICK: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoopEvent {
    StartGrain {
        duration: f64,
    }, // tell the grain player to start a grain
    StartLegatoGrain {
        duration: f64,
        offset_reduction: f64,
    }, // tell the grain player to start a grain part way thru, in the case where we want an existing grain to continue
    StopGrain, // stops the grain player
    LoopEnded, // a requested stop has committed at a grid boundary; looping is over
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LoopPhase {
    // not looping, nothing sounding
    Idle,
    // start requested at beat time `since`, waiting for the grid boundary
    Armed { since: f64 },
    // a grain is sounding and runs out at beat time `grain_end`
    Looping { grain_end: f64 },
    // stop requested: the loop ends at the next grid boundary
    Stopping { grain_end: f64 },
}

// what a call to start_looping() actually did, so the caller knows whether a
// new buffer reference is needed (Armed) or the old loop simply carries on
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartTransition {
    Armed,
    Resumed,
    Ignored,
}

// what a call to stop_looping_on_next_grid() actually did: CancelledArm means
// looping never got going, so there is nothing sounding to wind down
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StopTransition {
    StopScheduled,
    CancelledArm,
    Ignored,
}

pub struct LoopScheduler {
    phase: LoopPhase,
    fade_in_time: f64,
    grid_interval: f64,
    current_song_time: f64,
}

type BeatTime = f64;

// Beat time is now f64 end to end, but the grid interval and fade lead-in
// ultimately come from f32 parameters. Dividing an f64 song time by an
// f32-derived grid can leave a value that should sit exactly on a grid boundary
// an ulp above/below an integer, which would make ceil()/floor() jump a whole
// grid. This tolerance guards those boundaries so events still fire on the
// intended sample. It is applied inside the ceil/floor below (rather than by
// rounding the result back to f32, which would defeat the f64 precision this
// change exists to preserve) so the pure grid functions stay exact for callers
// that feed true f64 beat times.
const GRID_EPSILON: BeatTime = 1e-6;

// for a given song time, find the next grid interval according to the grid interval.
// The whole grid can be offset so that fades lead up to the grid.
fn next_grid_in_beats(
    song_time: BeatTime,
    grid_interval: BeatTime,
    grid_offset: BeatTime,
) -> BeatTime {
    ((song_time + grid_offset) / grid_interval - GRID_EPSILON).ceil() * grid_interval - grid_offset
}

fn previous_grid_in_beats(
    song_time: BeatTime,
    grid_interval: BeatTime,
    grid_offset: BeatTime,
) -> BeatTime {
    ((song_time + grid_offset) / grid_interval + GRID_EPSILON).floor() * grid_interval - grid_offset
}

impl LoopScheduler {
    pub fn new() -> LoopScheduler {
        LoopScheduler {
            phase: LoopPhase::Idle,
            fade_in_time: 0.0,
            grid_interval: 1.0,
            current_song_time: -1.0,
        }
    }

    pub fn reset(&mut self) {
        self.phase = LoopPhase::Idle;
        self.current_song_time = -1.0;
    }

    // set fade lead time in beats
    pub fn set_fade_lead_in(&mut self, fade_in: f64) {
        self.fade_in_time = fade_in;
    }

    // nothing needs rescheduling on a grid change: the next tick reconciles
    // the new boundaries against the grain that is actually sounding
    pub fn set_grid_interval(&mut self, new_interval_beats: f64) {
        self.grid_interval = new_interval_beats;
    }

    pub fn start_looping(&mut self) -> StartTransition {
        match self.phase {
            LoopPhase::Idle => {
                self.phase = LoopPhase::Armed {
                    since: self.current_song_time,
                };
                StartTransition::Armed
            }
            // a pending stop hasn't committed yet: cancel it and carry on
            // looping seamlessly, the sounding grain is still valid
            LoopPhase::Stopping { grain_end } => {
                self.phase = LoopPhase::Looping { grain_end };
                StartTransition::Resumed
            }
            LoopPhase::Armed { .. } | LoopPhase::Looping { .. } => StartTransition::Ignored,
        }
    }

    pub fn beats_since_last_grid(&self) -> f64 {
        let previous_grid_interval = previous_grid_in_beats(
            self.current_song_time,
            self.grid_interval,
            self.fade_in_time,
        );
        self.current_song_time - previous_grid_interval
    }

    pub fn stop_looping_on_next_grid(&mut self) -> StopTransition {
        match self.phase {
            LoopPhase::Looping { grain_end } => {
                self.phase = LoopPhase::Stopping { grain_end };
                StopTransition::StopScheduled
            }
            // never reached the first boundary: nothing is sounding
            LoopPhase::Armed { .. } => {
                self.phase = LoopPhase::Idle;
                StopTransition::CancelledArm
            }
            LoopPhase::Idle | LoopPhase::Stopping { .. } => StopTransition::Ignored,
        }
    }

    pub fn stop_looping_immediately(&mut self) {
        // the caller is expected to stop the grain player itself
        self.phase = LoopPhase::Idle;
    }

    pub fn is_looping(&self) -> bool {
        matches!(
            self.phase,
            LoopPhase::Armed { .. } | LoopPhase::Looping { .. }
        )
    }

    // true while loop content is actually sounding: from the first grain
    // firing (Armed doesn't count) until a stop commits at its boundary.
    // Survives a Stopping -> Looping resume
    pub fn is_committed(&self) -> bool {
        matches!(
            self.phase,
            LoopPhase::Looping { .. } | LoopPhase::Stopping { .. }
        )
    }

    pub fn tick(&mut self, beat_time: f64) -> ArrayVec<LoopEvent, MAX_EVENTS_PER_TICK> {
        if beat_time < self.current_song_time {
            // the transport jumped backwards (e.g. host loop wrap): keep any
            // sounding grain but re-anchor the next boundary to the new timeline
            self.phase = match self.phase {
                LoopPhase::Idle => LoopPhase::Idle,
                LoopPhase::Armed { .. } => LoopPhase::Armed { since: beat_time },
                LoopPhase::Looping { .. } => LoopPhase::Looping {
                    grain_end: self.next_boundary_after(beat_time),
                },
                LoopPhase::Stopping { .. } => LoopPhase::Stopping {
                    grain_end: self.next_boundary_after(beat_time),
                },
            };
        }

        let previous_time = self.current_song_time;
        self.current_song_time = beat_time;

        let mut events = ArrayVec::new();

        match self.phase {
            LoopPhase::Idle => {}
            LoopPhase::Armed { since } => {
                // the boundary at (or first after) the moment start was requested
                let start = next_grid_in_beats(since, self.grid_interval, self.fade_in_time);
                if beat_time >= start {
                    events.push(LoopEvent::StartGrain {
                        duration: self.grid_interval,
                    });
                    // the grain fires now; it runs out at the next boundary
                    // after the fire tick (`start` itself may be in the past)
                    self.phase = LoopPhase::Looping {
                        grain_end: self.next_boundary_after(beat_time),
                    };
                }
            }
            LoopPhase::Looping { grain_end } => {
                let next_boundary = self.next_boundary_after(previous_time);
                let boundary_crossed = next_boundary <= beat_time;

                if boundary_crossed && next_boundary < grain_end - GRID_EPSILON {
                    // grid was shortened: a boundary arrives while the current
                    // grain still has time to run - cut it and loop from here
                    events.push(LoopEvent::StopGrain);
                    events.push(LoopEvent::StartGrain {
                        duration: self.grid_interval,
                    });
                    self.phase = LoopPhase::Looping {
                        grain_end: next_boundary + self.grid_interval,
                    };
                } else if boundary_crossed && (next_boundary - grain_end).abs() <= GRID_EPSILON {
                    // the normal case: the grain runs out exactly on a boundary
                    events.push(LoopEvent::StartGrain {
                        duration: self.grid_interval,
                    });
                    self.phase = LoopPhase::Looping {
                        grain_end: next_boundary + self.grid_interval,
                    };
                } else if beat_time >= grain_end && grain_end < next_boundary - GRID_EPSILON {
                    // grid was lengthened: the grain runs out before the next
                    // boundary - bridge the gap with the tail of the loop content
                    let duration = next_boundary - grain_end;
                    events.push(LoopEvent::StartLegatoGrain {
                        duration,
                        offset_reduction: self.grid_interval - duration,
                    });
                    self.phase = LoopPhase::Looping {
                        grain_end: next_boundary,
                    };
                }
            }
            LoopPhase::Stopping { .. } => {
                let next_boundary = self.next_boundary_after(previous_time);
                if next_boundary <= beat_time {
                    events.push(LoopEvent::StopGrain);
                    events.push(LoopEvent::LoopEnded);
                    self.phase = LoopPhase::Idle;
                }
            }
        }

        events
    }

    // the first grid boundary strictly after `time`
    fn next_boundary_after(&self, time: f64) -> f64 {
        let boundary = next_grid_in_beats(time, self.grid_interval, self.fade_in_time);
        if boundary <= time {
            boundary + self.grid_interval
        } else {
            boundary
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_grid_in_beats() {
        assert_eq!(next_grid_in_beats(0.0, 1.0, 0.0), 0.0);

        assert_eq!(next_grid_in_beats(0.2, 1.0, 0.0), 1.0);
        assert_eq!(next_grid_in_beats(0.8, 1.0, 0.0), 1.0);

        assert_eq!(next_grid_in_beats(0.2, 1.0, -0.1), 1.1);

        // positive fade time gives an earlier grid
        assert_eq!(next_grid_in_beats(0.8, 1.0, 0.1), 0.9);

        // quarter beats
        assert_eq!(next_grid_in_beats(2.2, 0.25, 0.0), 2.25);
    }

    #[test]
    fn test_previous_grid_in_beats() {
        assert_eq!(previous_grid_in_beats(0.0, 1.0, 0.0), 0.0);

        assert_eq!(previous_grid_in_beats(0.2, 1.0, 0.0), 0.0);
        assert_eq!(previous_grid_in_beats(0.8, 1.0, 0.0), 0.0);

        assert_eq!(previous_grid_in_beats(0.2, 1.0, -0.1), 0.1);

        // positive fade time gives an earlier grid
        assert_eq!(previous_grid_in_beats(1.8, 1.0, 0.1), 0.9);

        // quarter beats
        assert_eq!(previous_grid_in_beats(3.3, 0.25, 0.0), 3.25);
    }

    #[test]
    fn test_loop_scheduler_simple_loop() {
        let mut scheduler = LoopScheduler::new();

        let grid = 1.0;

        let out0 = scheduler.tick(0.0);
        assert_eq!(out0.as_slice(), &[] as &[LoopEvent]);
        scheduler.set_grid_interval(grid);

        assert_eq!(scheduler.start_looping(), StartTransition::Armed);
        let out1 = scheduler.tick(1.0);
        assert_eq!(out1.as_slice(), &[LoopEvent::StartGrain { duration: grid }]);

        let out15 = scheduler.tick(1.5);
        assert_eq!(out15.as_slice(), &[] as &[LoopEvent]);

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2.as_slice(), &[LoopEvent::StartGrain { duration: grid }]);

        assert_eq!(
            scheduler.stop_looping_on_next_grid(),
            StopTransition::StopScheduled
        );
        let out3 = scheduler.tick(3.0);
        assert_eq!(
            out3.as_slice(),
            &[LoopEvent::StopGrain, LoopEvent::LoopEnded]
        );
        let out9 = scheduler.tick(9.0);
        assert_eq!(out9.as_slice(), &[] as &[LoopEvent]);
    }

    #[test]
    fn test_loop_scheduler_simple_loop_offset() {
        let mut scheduler = LoopScheduler::new();

        // a small offset should produce the same result as above
        scheduler.set_fade_lead_in(0.01);

        let grid = 1.0;

        let out0 = scheduler.tick(0.0);
        assert_eq!(out0.as_slice(), &[] as &[LoopEvent]);
        scheduler.set_grid_interval(grid);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(out1.as_slice(), &[LoopEvent::StartGrain { duration: grid }]);

        let out15 = scheduler.tick(1.5);
        assert_eq!(out15.as_slice(), &[] as &[LoopEvent]);

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2.as_slice(), &[LoopEvent::StartGrain { duration: grid }]);

        scheduler.stop_looping_on_next_grid();
        let out3 = scheduler.tick(3.0);
        assert_eq!(
            out3.as_slice(),
            &[LoopEvent::StopGrain, LoopEvent::LoopEnded]
        );
        let out9 = scheduler.tick(9.0);
        assert_eq!(out9.as_slice(), &[] as &[LoopEvent]);
    }

    #[test]
    fn test_loop_scheduler_shorten_loop() {
        // test the situation where we shorten a loop whilst a longer loop is playing
        let mut scheduler = LoopScheduler::new();

        let grid1 = 1.0;
        let grid2 = 0.5;

        let out0 = scheduler.tick(0.0);
        assert_eq!(out0.as_slice(), &[] as &[LoopEvent]);
        scheduler.set_grid_interval(grid1);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(out1.as_slice(), &[LoopEvent::StartGrain { duration: grid1 }]);

        let out15 = scheduler.tick(1.5);
        assert_eq!(out15.as_slice(), &[] as &[LoopEvent]);

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2.as_slice(), &[LoopEvent::StartGrain { duration: grid1 }]);

        let out225 = scheduler.tick(2.25);
        assert_eq!(out225.as_slice(), &[] as &[LoopEvent]);
        scheduler.set_grid_interval(grid2);

        // the next loop starts at 2.5, the existing one is stopped
        let out25 = scheduler.tick(2.5);
        assert_eq!(
            out25.as_slice(),
            &[
                LoopEvent::StopGrain,
                LoopEvent::StartGrain { duration: grid2 },
            ]
        );

        let out3 = scheduler.tick(3.0);
        assert_eq!(out3.as_slice(), &[LoopEvent::StartGrain { duration: grid2 }]);
    }

    #[test]
    fn test_loop_scheduler_lengthen_loop_early() {
        // this tests the "legato bridge" when the loop is lengthened very
        // soon after looping is started
        let mut scheduler = LoopScheduler::new();

        let grid1 = 1.0;
        let grid2 = 4.0;

        let out0 = scheduler.tick(0.0);
        assert_eq!(out0.as_slice(), &[] as &[LoopEvent]);
        scheduler.set_grid_interval(grid1);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(out1.as_slice(), &[LoopEvent::StartGrain { duration: grid1 }]);

        let out15 = scheduler.tick(1.5);
        assert_eq!(out15.as_slice(), &[] as &[LoopEvent]);

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2.as_slice(), &[LoopEvent::StartGrain { duration: grid1 }]);

        let out225 = scheduler.tick(2.25);
        assert_eq!(out225.as_slice(), &[] as &[LoopEvent]);
        scheduler.set_grid_interval(grid2);

        // when the short loop runs out, we get a "legato" grain that takes us
        // to the next (longer) grid interval. We need an extra offset of 3 to
        // make sure we're playing the end of the legato grain
        let out3 = scheduler.tick(3.0);
        assert_eq!(
            out3.as_slice(),
            &[LoopEvent::StartLegatoGrain {
                duration: 1.0,
                offset_reduction: 3.0
            }]
        );

        // then the new loop starts
        let out4 = scheduler.tick(4.0);
        assert_eq!(out4.as_slice(), &[LoopEvent::StartGrain { duration: grid2 }]);

        // and continues
        let out8 = scheduler.tick(8.0);
        assert_eq!(out8.as_slice(), &[LoopEvent::StartGrain { duration: grid2 }]);
    }

    #[test]
    fn test_loop_scheduler_lengthen_loop_late() {
        // as above but later, so no bridge is needed
        let mut scheduler = LoopScheduler::new();

        let grid1 = 1.0;
        let grid2 = 4.0;

        let out0 = scheduler.tick(0.0);
        assert_eq!(out0.as_slice(), &[] as &[LoopEvent]);
        scheduler.set_grid_interval(grid1);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(out1.as_slice(), &[LoopEvent::StartGrain { duration: grid1 }]);

        let out15 = scheduler.tick(1.5);
        assert_eq!(out15.as_slice(), &[] as &[LoopEvent]);

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2.as_slice(), &[LoopEvent::StartGrain { duration: grid1 }]);

        let out3 = scheduler.tick(3.0);
        assert_eq!(out3.as_slice(), &[LoopEvent::StartGrain { duration: grid1 }]);

        scheduler.tick(3.8);
        scheduler.set_grid_interval(grid2);

        // then the new loop starts
        let out4 = scheduler.tick(4.0);
        assert_eq!(out4.as_slice(), &[LoopEvent::StartGrain { duration: grid2 }]);

        // and continues
        let out8 = scheduler.tick(8.0);
        assert_eq!(out8.as_slice(), &[LoopEvent::StartGrain { duration: grid2 }]);
    }

    #[test]
    fn test_loop_scheduler_is_committed() {
        let mut scheduler = LoopScheduler::new();
        scheduler.set_grid_interval(1.0);
        scheduler.tick(0.2);

        // armed but nothing sounding yet
        scheduler.start_looping();
        assert!(!scheduler.is_committed());

        // cancelling the arm never commits
        scheduler.stop_looping_on_next_grid();
        scheduler.tick(2.0);
        assert!(!scheduler.is_committed());

        // committed once the first grain fires
        scheduler.start_looping();
        scheduler.tick(3.0);
        assert!(scheduler.is_committed());

        // survives a stop that gets cancelled before its boundary
        scheduler.stop_looping_on_next_grid();
        assert!(scheduler.is_committed());
        scheduler.tick(3.5);
        scheduler.start_looping();
        assert!(scheduler.is_committed());

        // ends when a stop commits at its boundary
        scheduler.stop_looping_on_next_grid();
        scheduler.tick(4.0);
        assert!(!scheduler.is_committed());
    }

    #[test]
    fn test_loop_scheduler_stop_before_first_boundary() {
        // a start followed by a stop before the boundary arrives never fires
        // anything: the arm is simply cancelled
        let mut scheduler = LoopScheduler::new();
        scheduler.set_grid_interval(1.0);
        scheduler.tick(0.2);

        assert_eq!(scheduler.start_looping(), StartTransition::Armed);
        assert!(scheduler.is_looping());
        let out = scheduler.tick(0.5);
        assert_eq!(out.as_slice(), &[] as &[LoopEvent]);

        assert_eq!(
            scheduler.stop_looping_on_next_grid(),
            StopTransition::CancelledArm
        );
        assert!(!scheduler.is_looping());

        // nothing ever fires, dry was never touched
        for t in [1.0, 2.0, 3.0] {
            assert_eq!(scheduler.tick(t).as_slice(), &[] as &[LoopEvent]);
        }
    }

    #[test]
    fn test_loop_scheduler_restart_before_stop_boundary_resumes() {
        // stop followed by a start before the stop's boundary cancels the stop:
        // the loop carries on as if never released, no StopGrain is emitted
        let mut scheduler = LoopScheduler::new();
        scheduler.set_grid_interval(1.0);
        scheduler.tick(0.0);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(out1.as_slice(), &[LoopEvent::StartGrain { duration: 1.0 }]);

        scheduler.stop_looping_on_next_grid();
        scheduler.tick(1.5);
        assert_eq!(scheduler.start_looping(), StartTransition::Resumed);

        // the loop continues seamlessly at the next boundary
        let out2 = scheduler.tick(2.0);
        assert_eq!(out2.as_slice(), &[LoopEvent::StartGrain { duration: 1.0 }]);
    }

    #[test]
    fn test_loop_scheduler_rapid_grid_changes_reconcile() {
        // several grid changes between boundaries: only the last one matters,
        // the machine reconciles against the sounding grain rather than
        // clearing and rescheduling on each change
        let mut scheduler = LoopScheduler::new();
        scheduler.set_grid_interval(1.0);
        scheduler.tick(0.0);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(out1.as_slice(), &[LoopEvent::StartGrain { duration: 1.0 }]);

        // flail around between boundaries
        scheduler.set_grid_interval(0.25);
        scheduler.set_grid_interval(4.0);
        scheduler.set_grid_interval(0.5);

        // grid is now 0.5: a boundary arrives at 1.5, before the grain's own
        // end at 2.0 - the grain is cut and the loop restarts
        let out15 = scheduler.tick(1.5);
        assert_eq!(
            out15.as_slice(),
            &[
                LoopEvent::StopGrain,
                LoopEvent::StartGrain { duration: 0.5 },
            ]
        );

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2.as_slice(), &[LoopEvent::StartGrain { duration: 0.5 }]);
    }
}
