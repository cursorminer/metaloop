use crate::ramped_value::RampedValue;

// a rather short lived thing that plays a single faded grain
// the duration includes two fade durations
pub struct Grain {
    scheduled_wait: usize,
    delay_pos: usize,
    end_delay: usize,
    duration: usize,
    fade_duration: usize,
    elapsed_sample_count: usize,
    offset: usize,
    sample_increment: isize,
    fade_ramp: RampedValue,
}

#[allow(dead_code)]
impl Grain {
    // offset: the initial delay time where the grain starts
    // duration: how long the grain lasts
    // fade: number of samples to fade in and out (this is within the duration above)
    pub fn new(
        scheduled_wait: usize,
        offset: usize,
        duration: usize,
        fade: usize,
        reverse: bool,
    ) -> Grain {
        assert!(offset >= duration);

        let actual_fade = if (fade * 2) > duration {
            duration / 2
        } else {
            fade
        };

        let start_delay = if reverse {
            offset - duration - 1
        } else {
            offset
        };

        let end_delay = if reverse {
            offset - 1
        } else {
            offset - duration
        };

        let sample_increment = if reverse { -1 } else { 1 };

        Grain {
            scheduled_wait: scheduled_wait,
            delay_pos: start_delay,
            end_delay: end_delay,
            duration: duration,
            fade_duration: actual_fade,
            elapsed_sample_count: 0,
            offset: offset,
            sample_increment: sample_increment,
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

        if self.elapsed_sample_count == 0 && self.scheduled_wait == 0 {
            self.fade_ramp.set(0.0);
            self.fade_ramp.ramp(1.0, self.fade_duration);
        } else if self.elapsed_sample_count == (self.duration - self.fade_duration) {
            self.fade_ramp.set(1.0);
            self.fade_ramp.ramp(0.0, self.fade_duration);
        }
        let del = (self.delay_pos as isize) - self.sample_increment;
        assert!(del >= 0);

        self.delay_pos = del as usize;
        self.elapsed_sample_count = self.elapsed_sample_count + 1;

        let win = self.fade_ramp.tick();
        (self.delay_pos, win)
    }

    pub fn stop(&mut self) {
        // if already fading out don't stop it
        if self.elapsed_sample_count > (self.duration - self.fade_duration) {
            return;
        }

        // otherwise tweak the values so that the grain fades now
        self.duration = self.elapsed_sample_count + self.fade_duration;
        self.end_delay = self.delay_pos - self.fade_duration;
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
        return self.elapsed_sample_count < self.fade_duration;
    }
    pub fn is_fading_out(&self) -> bool {
        return self.elapsed_sample_count > (self.duration - self.fade_duration);
    }
    pub fn elapsed_sample_count(&self) -> usize {
        return self.elapsed_sample_count;
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
        let mut grain = Grain::new(0, 10, 5, 0, false);

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
        let mut grain = Grain::new(1, 10, 5, 0, false);

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
        let mut grain = Grain::new(0, 10, 9, 3, false);

        let expected = vec![
            (9, 0.25),
            (8, 0.5),
            (7, 0.75),
            (6, 1.0),
            (5, 1.0),
            (4, 1.0),
            (3, 0.75),
            (2, 0.5),
            (1, 0.25),
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
        let mut grain = Grain::new(0, 20, 15, 3, false);

        let expected = vec![(19, 0.25), (18, 0.5), (17, 0.75), (16, 1.0), (15, 1.0)];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);

        // stopping the grain should fade it out
        grain.stop();
        let expected_fade = vec![(14, 0.75), (13, 0.5), (12, 0.25), (0, 0.0), (0, 0.0)];

        let mut out = vec![];
        for _i in 0..expected_fade.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected_fade);
        assert_eq!(grain.is_finished(), true);
    }

    #[test]
    fn test_grain_reverse() {
        let mut grain = Grain::new(0, 10, 5, 0, true);

        let expected = vec![(5, 1.0), (6, 1.0), (7, 1.0), (8, 1.0), (9, 1.0), (0, 0.0)];
        let mut out = vec![];
        for _i in 0..expected.len() {
            // assert!(!grain.is_finished());
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
        assert!(grain.is_finished());
    }

    #[test]
    fn test_grain_fade_reverse() {
        let mut grain = Grain::new(0, 10, 9, 3, true);

        let expected = vec![
            (1, 0.25),
            (2, 0.5),
            (3, 0.75),
            (4, 1.0),
            (5, 1.0),
            (6, 1.0),
            (7, 0.75),
            (8, 0.5),
            (9, 0.25),
            (0, 0.0),
        ];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
    }
}
