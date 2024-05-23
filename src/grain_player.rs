use crate::delay_line::{self, DelayLine};
use crate::grain::Grain;
use crate::scheduled_grain::ScheduledGrain;

pub struct GrainPlayer {
    sample_rate: f32,
    grains: Vec<Grain>,
    fade_duration: usize,
}

// schedule and play grains
#[allow(dead_code)]
impl GrainPlayer {
    pub fn new(sample_rate: f32) -> GrainPlayer {
        GrainPlayer {
            sample_rate,
            grains: vec![],
            fade_duration: 0,
        }
    }

    pub fn set_fade_time(&mut self, fade: usize) {
        self.fade_duration = fade;
    }

    pub fn schedule_grain(&mut self, wait: usize, offset: usize, duration: usize) {
        let grain = Grain::new(wait, offset, duration, self.fade_duration);

        // replace a finished grain
        for i in 0..self.grains.len() {
            if self.grains[i].is_finished() {
                self.grains[i] = grain;
                return;
            }
        }
    }

    pub fn tick(&mut self, delay_line: &DelayLine, rolling_offset: usize) -> f32 {
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
            out += delay_line.read(delay_pos + rolling_offset) * amplitude;
        }
        out
    }

    fn num_scheduled_grains(&self) -> usize {
        let mut count = 0;
        for grain in self.grains.iter() {
            if grain.is_scheduled() {
                count += 1;
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grain_player() {
        let mut player = GrainPlayer::new(44100.0);
        let delay_line = DelayLine::new(44100);
        let out = player.tick(delay_line, 0);
        assert_eq!(out, 0.0);
        // fill the delay line with a constant value
        player.schedule_grain(2, 10, 4);
        assert_eq!(player.num_scheduled_grains(), 1);
    }
}
