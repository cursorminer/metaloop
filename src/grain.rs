use crate::ramped_value::RampedValue;

// a rather short lived thing that plays a single faded grain
// the duration includes two fade durations
pub struct Grain {
    scheduled_wait: usize,
    delay_pos: usize,
    end_delay: usize,
    duration: usize,
    fade_duration: usize,
    play_head_pos: usize,
    offset: usize,
    fade_ramp: RampedValue,
}

#[allow(dead_code)]
impl Grain {
    // offset: the initial delay time where the grain starts
    // duration: how long the grain lasts
    // fade: number of samples to fade in and out (this is within the duration above)
    pub fn new(scheduled_wait: usize, offset: usize, duration: usize, fade: usize) -> Grain {
        assert!(offset >= duration);

        let actual_fade = if (fade * 2) > duration {
            duration / 2
        } else {
            fade
        };

        Grain {
            scheduled_wait: scheduled_wait,
            delay_pos: offset,
            end_delay: offset - duration,
            duration: duration,
            fade_duration: actual_fade,
            play_head_pos: 0,
            offset: offset,
            fade_ramp: RampedValue::new(1.0),
        }
    }

    pub fn tick(&mut self) -> (usize, f32) {
        if self.is_finished() {
            return (0, 0.0);
        }

        if self.is_waiting() {
            self.scheduled_wait = self.scheduled_wait - 1;
            return (0, 0.0);
        }

        if self.play_head_pos == 0 && self.scheduled_wait == 0 {
            self.fade_ramp.ramp(1.0, self.fade_duration);
        } else if self.play_head_pos > (self.duration - self.fade_duration) {
            self.fade_ramp.ramp(0.0, self.fade_duration);
        }

        self.delay_pos = self.delay_pos - 1;
        self.play_head_pos = self.play_head_pos + 1;

        let win = self.fade_ramp.tick();
        (self.delay_pos, win)
    }

    pub fn stop(&mut self) {
        // if already fading out don't stop it
        if self.play_head_pos > (self.duration - self.fade_duration) {
            return;
        }

        // otherwise tweak the values so that the grain fades now
        self.duration = self.play_head_pos + self.fade_duration;
        self.end_delay = self.delay_pos + self.fade_duration;
    }

    pub fn is_finished(&self) -> bool {
        return self.delay_pos == self.end_delay || self.duration == 0;
    }

    pub fn is_waiting(&self) -> bool {
        return self.scheduled_wait > 0;
    }

    pub fn is_playing(&self) -> bool {
        return !self.is_finished() && !self.is_waiting();
    }

    pub fn is_fading_in(&self) -> bool {
        return self.play_head_pos < self.fade_duration;
    }
    pub fn is_fading_out(&self) -> bool {
        return self.play_head_pos > (self.duration - self.fade_duration);
    }
    pub fn play_head_pos(&self) -> usize {
        return self.play_head_pos;
    }
    pub fn offset(&self) -> usize {
        return self.offset;
    }
    pub fn duration(&self) -> usize {
        return self.duration;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grain() {
        let mut grain = Grain::new(0, 10, 5, 0);

        let expected = vec![(9, 1.0), (8, 1.0), (7, 1.0), (6, 1.0), (5, 1.0), (0, 0.0)];
        let mut out = vec![];
        for _i in 0..expected.len() {
            // assert!(!grain.is_finished());
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
        assert!(grain.is_finished());
    }

    #[test]
    fn test_grain_wait() {
        let mut grain = Grain::new(1, 10, 5, 0);

        let expected = vec![
            (0, 0.0),
            (9, 1.0),
            (8, 1.0),
            (7, 1.0),
            (6, 1.0),
            (5, 1.0),
            (0, 0.0),
        ];
        let mut out = vec![];
        for _i in 0..expected.len() {
            // assert!(!grain.is_finished());
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
        assert!(grain.is_finished());
    }

    #[test]
    fn test_grain_fade() {
        let mut grain = Grain::new(0, 10, 9, 4);

        let expected = vec![
            (9, 0.2),
            (8, 0.4),
            (7, 0.6),
            (6, 0.8),
            (5, 1.0),
            (4, 1.0),
            (3, 0.8),
            (2, 0.6),
            (1, 0.4),
            (0, 0.2),
            (0, 0.0),
        ];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
    }

    #[test]
    fn test_grain_stop() {
        let mut grain = Grain::new(0, 20, 15, 4);

        let expected = vec![(19, 0.25), (18, 0.5), (17, 0.75), (16, 1.0), (15, 1.0)];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);

        // stopping the grain should fade it out
        grain.stop();
        let expected_fade = vec![(14, 1.0), (13, 0.75), (12, 0.5), (11, 0.25), (10, 0.0)];

        let mut out = vec![];
        for _i in 0..expected_fade.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected_fade);
    }
}
