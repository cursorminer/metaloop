// This handles the actual events that control what the looper does
// according to the beat time
use crate::scheduler::Scheduler;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoopEvent {
    StartGrain {
        duration: f32,
    }, // tell the grain player to start a grain
    StartLegatoGrain {
        duration: f32,
        offset_reduction: f32,
    }, // tell the grain player to start a grain part way thru
    StopGrain,  // stops the grain player
    FadeOutDry, // fade out the dry signal
    FadeInDry,  // fade in the dry signal
    NextLoop,   // start the next loop, recurs
}
pub struct LoopScheduler {
    scheduler: Scheduler<LoopEvent>,
    fade_in_time: f32,
    grid_interval: f32,
    current_song_time: f32,
    time_looping_initiated: f32,
    is_looping: bool,
}

type BeatTime = f32; // might wanna have f64

fn next_grid_in_beats(
    song_time: BeatTime,
    grid_interval: BeatTime,
    grid_offset: BeatTime,
) -> BeatTime {
    ((song_time + grid_offset) / grid_interval).ceil() * grid_interval - grid_offset
}

fn previous_grid_in_beats(
    song_time: BeatTime,
    grid_interval: BeatTime,
    grid_offset: BeatTime,
) -> BeatTime {
    ((song_time + grid_offset) / grid_interval).floor() * grid_interval - grid_offset
}

impl LoopScheduler {
    pub fn new() -> LoopScheduler {
        LoopScheduler {
            scheduler: Scheduler::new(),
            fade_in_time: 0.0,
            grid_interval: 1.0,
            current_song_time: -1.0,
            time_looping_initiated: 0.0,
            is_looping: false,
        }
    }

    pub fn reset(&mut self) {
        self.scheduler.clear();
        self.is_looping = false;
    }

    // set fade lead time in beats
    pub fn set_fade_lead_in(&mut self, fade_in: f32) {
        // Do nothing
        self.fade_in_time = fade_in;
    }

    pub fn set_grid_interval(&mut self, new_interval: f32) {
        if new_interval == self.grid_interval || !self.is_looping {
            self.grid_interval = new_interval;
            return;
        }
        self.scheduler.clear();
        let next_old_grid_interval = next_grid_in_beats(
            self.current_song_time,
            self.grid_interval,
            self.fade_in_time,
        );
        let next_new_grid_interval =
            next_grid_in_beats(self.current_song_time, new_interval, self.fade_in_time);

        if new_interval < self.grid_interval {
            // if a shorter interval, need to stop the current grain
            self.scheduler
                .schedule_event(next_new_grid_interval, LoopEvent::StopGrain);
        } else if new_interval > self.grid_interval {
            if next_new_grid_interval > next_old_grid_interval {
                // need a grain that will take us to the longer grid interval from the end of the shorter
                let reduced_grid_interval = next_new_grid_interval - next_old_grid_interval;
                let how_far_thru = new_interval - reduced_grid_interval;
                self.scheduler.schedule_event(
                    next_old_grid_interval,
                    LoopEvent::StartLegatoGrain {
                        duration: reduced_grid_interval,
                        offset_reduction: how_far_thru,
                    },
                );
            }
        }
        self.scheduler
            .schedule_event(next_new_grid_interval, LoopEvent::NextLoop);
        self.grid_interval = new_interval;
    }

    pub fn start_looping(&mut self) {
        assert!(!self.is_looping);
        self.is_looping = true;
        self.time_looping_initiated = self.current_song_time;
        // schedule a fade out
        // schedule a grain to start at the next grid interval
        let next_grid_interval = next_grid_in_beats(
            self.current_song_time,
            self.grid_interval,
            self.fade_in_time,
        );

        self.scheduler
            .schedule_event(next_grid_interval, LoopEvent::NextLoop);
        self.scheduler
            .schedule_event(next_grid_interval, LoopEvent::FadeOutDry);
    }

    pub fn stop_looping(&mut self) {
        assert!(self.is_looping);
        self.is_looping = false;
        // schedule a fade in
        // schedule a grain to stop at the next grid interval
        let next_grid_interval = next_grid_in_beats(
            self.current_song_time,
            self.grid_interval,
            self.fade_in_time,
        );

        self.scheduler.clear();

        self.scheduler
            .schedule_event(next_grid_interval, LoopEvent::StopGrain);
        self.scheduler
            .schedule_event(next_grid_interval, LoopEvent::FadeInDry);
    }

    pub fn tick(&mut self, beat_time: f32) -> Vec<LoopEvent> {
        assert!(beat_time > self.current_song_time, "Time must go forward!");

        self.current_song_time = beat_time;

        let events = self.scheduler.tick(beat_time);
        let mut returned_events = vec![];
        for event in events {
            match event {
                LoopEvent::NextLoop => {
                    // record when we started the thing
                    returned_events.push(LoopEvent::StartGrain {
                        duration: self.grid_interval,
                    });
                    // schedule the next loop
                    self.scheduler.schedule_event(
                        self.current_song_time + self.grid_interval,
                        LoopEvent::NextLoop,
                    );
                }
                _ => {
                    returned_events.push(event);
                }
            }
        }

        returned_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_grid_in_beats() {
        assert_eq!(next_grid_in_beats(0.0, 1.0, 0.0), 0.0);
    }

    #[test]
    fn test_loop_scheduler_simple_loop() {
        let mut scheduler = LoopScheduler::new();

        let grid = 1.0;

        let out0 = scheduler.tick(0.0);
        assert_eq!(out0, vec![]);
        scheduler.set_grid_interval(grid);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(
            out1,
            vec![
                LoopEvent::StartGrain { duration: grid },
                LoopEvent::FadeOutDry
            ]
        );

        let out15 = scheduler.tick(1.5);
        assert_eq!(out15, vec![]);

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2, vec![LoopEvent::StartGrain { duration: grid }]);

        scheduler.stop_looping();
        let out2 = scheduler.tick(3.0);
        assert_eq!(out2, vec![LoopEvent::StopGrain, LoopEvent::FadeInDry]);
        let out9 = scheduler.tick(9.0);
        assert_eq!(out9, vec![]);
    }

    #[test]
    fn test_loop_scheduler_shorten_loop() {
        let mut scheduler = LoopScheduler::new();

        let grid1 = 1.0;
        let grid2 = 0.5;

        let out0 = scheduler.tick(0.0);
        assert_eq!(out0, vec![]);
        scheduler.set_grid_interval(grid1);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(
            out1,
            vec![
                LoopEvent::StartGrain { duration: grid1 },
                LoopEvent::FadeOutDry
            ]
        );

        let out15 = scheduler.tick(1.5);
        assert_eq!(out15, vec![]);

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2, vec![LoopEvent::StartGrain { duration: grid1 }]);

        let out225 = scheduler.tick(2.25);
        assert_eq!(out225, vec![]);
        scheduler.set_grid_interval(grid2);

        // the next loop starts at 2.5, the existing one is stopped
        let out25 = scheduler.tick(2.5);
        assert_eq!(
            out25,
            vec![
                LoopEvent::StopGrain,
                LoopEvent::StartGrain { duration: grid2 }
            ]
        );
    }

    #[test]
    fn test_loop_scheduler_lengthen_loop_early() {
        // this tests the "back to dry" when the loop is lengthened very
        // soon after looping is started
        let mut scheduler = LoopScheduler::new();

        let grid1 = 1.0;
        let grid2 = 4.0;

        let out0 = scheduler.tick(0.0);
        assert_eq!(out0, vec![]);
        scheduler.set_grid_interval(grid1);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(
            out1,
            vec![
                LoopEvent::StartGrain { duration: grid1 },
                LoopEvent::FadeOutDry
            ]
        );

        let out15 = scheduler.tick(1.5);
        assert_eq!(out15, vec![]);

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2, vec![LoopEvent::StartGrain { duration: grid1 }]);

        let out225 = scheduler.tick(2.25);
        assert_eq!(out225, vec![]);
        scheduler.set_grid_interval(grid2);

        // when the short loop stops, we get a "legato" grain that takes us to the next interval
        // we need an extra offset of 3 to make sure we're playing the end of the
        // legato grain
        let out25 = scheduler.tick(3.0);
        assert_eq!(
            out25,
            vec![LoopEvent::StartLegatoGrain {
                duration: 1.0,
                offset_reduction: 3.0
            }]
        );

        // then the new loop starts
        let out4 = scheduler.tick(4.0);
        assert_eq!(out4, vec![LoopEvent::StartGrain { duration: grid2 }]);

        // and continues
        let out5 = scheduler.tick(8.0);
        assert_eq!(out5, vec![LoopEvent::StartGrain { duration: grid2 }]);
    }

    #[test]
    fn test_loop_scheduler_lengthen_loop_late() {
        // as above but later, so the dry is not needed
        let mut scheduler = LoopScheduler::new();

        let grid1 = 1.0;
        let grid2 = 4.0;

        let out0 = scheduler.tick(0.0);
        assert_eq!(out0, vec![]);
        scheduler.set_grid_interval(grid1);

        scheduler.start_looping();
        let out1 = scheduler.tick(1.0);
        assert_eq!(
            out1,
            vec![
                LoopEvent::StartGrain { duration: grid1 },
                LoopEvent::FadeOutDry
            ]
        );

        let out15 = scheduler.tick(1.5);
        assert_eq!(out15, vec![]);

        let out2 = scheduler.tick(2.0);
        assert_eq!(out2, vec![LoopEvent::StartGrain { duration: grid1 }]);

        let out3 = scheduler.tick(3.0);
        assert_eq!(out3, vec![LoopEvent::StartGrain { duration: grid1 }]);

        scheduler.tick(3.8);
        scheduler.set_grid_interval(grid2);

        // then the new loop starts
        let out4 = scheduler.tick(4.0);
        assert_eq!(out4, vec![LoopEvent::StartGrain { duration: grid2 }]);

        // and continues
        let out8 = scheduler.tick(8.0);
        assert_eq!(out8, vec![LoopEvent::StartGrain { duration: grid2 }]);
    }
}
