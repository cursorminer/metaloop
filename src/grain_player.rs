use crate::delay_line::{self, DelayLine};
use crate::grain::Grain;
use crate::scheduled_grain::ScheduledGrain;

pub const MAX_GRAINS: usize = 10;

pub struct GrainPlayer {
    sample_rate: f32,
    grains: Vec<Grain>,
    fade_duration: usize,
}

// schedule and play grains
#[allow(dead_code)]
impl GrainPlayer {
    pub fn new(sample_rate: f32) -> GrainPlayer {
        let mut grains_init = vec![];
        for _ in 0..MAX_GRAINS {
            grains_init.push(Grain::new(0, 0, 0, 0));
        }

        GrainPlayer {
            sample_rate,
            grains: grains_init,
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
            if grain.is_waiting() {
                grain.tick();
                continue;
            }
            let (delay_pos, amplitude) = grain.tick();
            out += delay_line.read(delay_pos + rolling_offset) * amplitude;
        }
        out
    }

    fn num_scheduled_grains(&self) -> usize {
        self.grains
            .iter()
            .filter(|grain| grain.is_waiting())
            .count()
    }

    fn num_playing_grains(&self) -> usize {
        self.grains
            .iter()
            .filter(|grain| !grain.is_finished() && !grain.is_waiting())
            .count()
    }

    fn num_finished_grains(&self) -> usize {
        self.grains
            .iter()
            .filter(|grain| grain.is_finished())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grain_player() {
        let mut player = GrainPlayer::new(44100.0);
        let delay_line = DelayLine::new(44100);
        let out = player.tick(&delay_line, 0);

        assert_eq!(out, 0.0);

        player.schedule_grain(2, 10, 4);

        assert_eq!(player.num_scheduled_grains(), 1);
        assert_eq!(player.num_playing_grains(), 0);
        assert_eq!(player.num_finished_grains(), MAX_GRAINS - 1);

        // tick past wait time
        for _ in 0..2 {
            player.tick(&delay_line, 0);
        }

        assert_eq!(player.num_scheduled_grains(), 0);
        assert_eq!(player.num_playing_grains(), 1);
        assert_eq!(player.num_finished_grains(), MAX_GRAINS - 1);

        // tick past duration
        for _ in 0..4 {
            player.tick(&delay_line, 0);
        }
        assert_eq!(player.num_scheduled_grains(), 0);
        assert_eq!(player.num_playing_grains(), 0);
        assert_eq!(player.num_finished_grains(), MAX_GRAINS);
    }
}
