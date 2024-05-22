use crate::delay_line::{self, DelayLine};
use crate::grain::Grain;
use crate::scheduled_grain::ScheduledGrain;

struct GrainPlayer {
    delay_line: DelayLine,
    sample_rate: f32,
    grains: Vec<Grain>,
    fade_duration: usize,
}

// schedule and play grains
#[allow(dead_code)]
impl GrainPlayer {
    pub fn new(sample_rate: f32) -> GrainPlayer {
        let delay_line = DelayLine::new(44100);
        GrainPlayer {
            delay_line,
            sample_rate,
            grains: vec![],
            fade_duration: 0,
        }
    }

    pub fn set_fade_time(&mut self, seconds: f32) {
        self.fade_duration = (seconds * self.sample_rate) as usize;
    }

    pub fn schedule_grain(&mut self, wait: f32, offset: f32, duration: f32) {
        let grain = Grain::new(
            (wait * self.sample_rate) as usize,
            (offset * self.sample_rate) as usize,
            (duration * self.sample_rate) as usize,
            self.fade_duration,
        );

        // replace a finished grain
        for i in 0..self.grains.len() {
            if self.grains[i].is_finished() {
                self.grains[i] = grain;
                return;
            }
        }
    }

    pub fn tick(&mut self) -> f32 {
        let mut out = 0.0;

        // accumulate output of all grains
        for grain in self.grains.iter_mut() {
            if grain.is_finished() {
                continue;
            }
            if grain.is_scheduled() {
                grain.tick();
                continue;
            }
            let (delay_pos, amplitude) = grain.tick();
            out += self.delay_line.read(delay_pos) * amplitude;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grain_player() {
        let mut player = GrainPlayer::new(44100.0);
        let out = player.tick();
        assert_eq!(out, 0.0);
    }
}
