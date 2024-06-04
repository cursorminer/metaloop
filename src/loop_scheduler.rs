// This handles the actual events that control what the looper does
// according to the beat time
use crate::scheduler::Scheduler;

enum LoopEvent {
    StartGrain,
    StopGrain,
    FadeOutDry,
    FadeInDry,
}
pub struct LoopScheduler 
{
    scheduler: Scheduler<LoopEvent>,
    fade_in_time: f32,
    grid_interval: f32,
    current_beat_time: f32,
}

use BeatTime = f32; // might wanna have f64

fn next_grid_in_beats(song_time: BeatTime, grid_interval: BeatTime, grid_offset: BeatTime) -> BeatTime
{
  ceil((song_time + grid_offset) / grid_interval) * grid_interval
         - grid_offset;
}

fn previous_grid_in_beats(song_time: BeatTime, grid_interval: BeatTime,  grid_offset: BeatTime) -> BeatTime
{
  return floor((song_time + grid_offset) / grid_interval) * grid_interval
         - grid_offset;
}

impl LoopScheduler {
    pub fn new() -> LoopScheduler {
        LoopScheduler {}
    }

    // set fade lead time in beats
    pub fn set_fade_lead_in(&mut self, fade_in: f32) {
        // Do nothing
        self.fade_in_time = fade_in;
    }

    pub fn set_grid_interval(&mut self, interval: f32) {
        // Do nothing
    }

    pub fn start_looping(&mut self) {
        // schedule a fade out
        // schedule a grain to start at the next grid interval
        let next_grid_interval = get_next_grid_interval(self.current_beat_time);
    }

    pub fn tick(&mut self, beat_time: f32) {
        // Do nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_scheduler_simple_loop() {
        let mut scheduler = LoopScheduler::new();
        scheduler.set_grid_interval(interval: 1.0);
        scheduler.start_looping();
        scheduler.tick(0.0);
    }
}
