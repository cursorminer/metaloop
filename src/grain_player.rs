use crate::grain::Grain;
use crate::{delay_line::DelayLine, stereo_pair::AudioSampleOps};

pub const MAX_GRAINS: usize = 10;

pub struct GrainPlayer {
    grains: Vec<Grain>,
    fade_duration: usize,
    reverse: bool,
    speed: f32,
}

// schedule and play grains
#[allow(dead_code)]
impl GrainPlayer {
    pub fn new() -> GrainPlayer {
        let mut grains_init = vec![];
        for _ in 0..MAX_GRAINS {
            grains_init.push(Grain::new(0, 0.0, 0, 0, false, 0.0));
        }

        GrainPlayer {
            grains: grains_init,
            fade_duration: 0,
            reverse: false,
            speed: 1.0,
        }
    }

    pub fn set_fade_time(&mut self, fade: usize) {
        self.fade_duration = fade;
    }
    pub fn set_reverse(&mut self, reverse: bool) {
        self.reverse = reverse;
    }
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn schedule_grain(&mut self, wait: usize, offset: f32, duration: usize) {
        let grain = Grain::new(
            wait,
            offset,
            duration,
            self.fade_duration,
            self.reverse,
            self.speed,
        );

        // replace a finished grain
        for i in 0..self.grains.len() {
            if self.grains[i].is_finished() {
                self.grains[i] = grain;
                return;
            }
        }
    }

    pub fn tick<T: AudioSampleOps>(
        &mut self,
        delay_line: &DelayLine<T>,
        rolling_offset: usize,
    ) -> T {
        let mut out = Default::default();

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
            let delay = delay_pos + rolling_offset as f32;
            assert!(
                delay >= 0.0 && delay < delay_line.len() as f32,
                "delay is outside buffer. delay_pos: {:?}, rolling_offset: {:?}",
                delay_pos,
                rolling_offset
            );

            out = out + delay_line.read_interpolated(delay) * amplitude;
        }
        out
    }

    // todo: alternative tick that can loop over a delay line of
    // things that can't be interpolated or whatnot  might need different impl

    pub fn stop_all_grains(&mut self) {
        for grain in self.grains.iter_mut() {
            grain.stop();
        }
    }

    fn num_scheduled_grains(&self) -> usize {
        self.grains
            .iter()
            .filter(|grain| grain.is_waiting())
            .count()
    }

    pub fn num_playing_grains(&self) -> usize {
        self.grains
            .iter()
            .filter(|grain| grain.is_playing())
            .count()
    }

    fn num_finished_grains(&self) -> usize {
        self.grains
            .iter()
            .filter(|grain| grain.is_finished())
            .count()
    }

    pub fn most_recent_grain(&self) -> Option<&Grain> {
        self.grains
            .iter()
            .filter(|grain| grain.is_playing())
            .min_by_key(|grain| grain.elapsed_sample_count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delay_line::fill_delay_ramp;

    #[test]
    fn test_grain_player_state() {
        let mut player = GrainPlayer::new();
        let delay_line: DelayLine<f32> = DelayLine::new(100);

        player.schedule_grain(2, 10.0, 4);

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

    #[test]
    fn test_grain_player_stop_all() {
        let mut player = GrainPlayer::new();
        let delay_line: DelayLine<f32> = DelayLine::new(100);
        player.set_fade_time(2);

        player.schedule_grain(0, 10.0, 4);
        player.schedule_grain(0, 10.0, 10);

        assert_eq!(player.num_playing_grains(), 2);

        player.tick(&delay_line, 0);

        assert_eq!(player.num_playing_grains(), 2);

        player.stop_all_grains();

        player.tick(&delay_line, 0);

        // grains keep going until fade is finished
        assert_eq!(player.num_playing_grains(), 2);

        player.tick(&delay_line, 0);
        player.tick(&delay_line, 0);

        assert_eq!(player.num_playing_grains(), 0);
        assert_eq!(player.num_finished_grains(), 10);
    }

    #[test]
    fn test_grain_player_output() {
        let mut player = GrainPlayer::new();
        let mut delay_line: DelayLine<f32> = DelayLine::new(20);
        fill_delay_ramp(&mut delay_line);
        let mut out = vec![];

        player.schedule_grain(2, 10.0, 4);

        // tick past wait time
        for _ in 0..2 {
            out.push(player.tick(&delay_line, 0));
        }

        // tick past duration
        for _ in 0..5 {
            out.push(player.tick(&delay_line, 0));
        }

        assert_eq!(out, vec![0.0, 0.0, 10.0, 11.0, 12.0, 13.0, 0.0]);

        out.clear();
        player.set_fade_time(1);
        player.schedule_grain(2, 10.0, 4);

        // tick past wait time
        for _ in 0..2 {
            out.push(player.tick(&delay_line, 0));
        }

        // tick past duration
        for _ in 0..5 {
            out.push(player.tick(&delay_line, 0));
        }
        // as above but one sample is faded
        assert_eq!(out, vec![0.0, 0.0, 5.0, 11.0, 12.0, 6.5, 0.0]);
    }
}
