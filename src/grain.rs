use crate::ramped_value::RampedValue;

// Q: it would be nice if we could support the cases where fractional delays make sense
// and when it doesn't

// a rather short lived thing that plays a single faded grain
// the duration includes two fade durations
pub struct Grain {
    scheduled_wait: usize,       // how long to wait before starting
    delay_pos: f32,              // current delay position, ticks *down* to read forwards
    duration: usize,             // how long the grain lasts in ticks
    fade_duration: usize,        // how many samples to fade over (in and out)
    elapsed_sample_count: usize, // how many samples have been output
    offset: f32,                 // the initial delay time where the grain starts
    sample_increment: f32,       // how much to increment the delay position each tick
    fade_ramp: RampedValue,      // the fade in/out ramp
}

#[allow(dead_code)]
impl Grain {
    // offset: the initial delay time where the grain starts
    // duration: how long the grain lasts
    // fade: number of samples to fade in and out (this is within the duration above)
    // speed: how fast to play the grain, 1 is normal, 0.5 is half speed
    pub fn new(
        scheduled_wait: usize,
        offset: f32,
        duration: usize,
        fade: usize,
        reverse: bool,
        speed: f32,
    ) -> Grain {
        let actual_fade = if (fade * 2) > duration {
            duration / 2
        } else {
            fade
        };

        let start_delay = if reverse {
            offset - duration as f32
        } else {
            offset - 1.0
        };

        let sample_increment = if reverse { -speed } else { speed };

        Grain {
            scheduled_wait: scheduled_wait,
            delay_pos: start_delay,
            duration: duration,
            fade_duration: actual_fade,
            elapsed_sample_count: 0,
            offset: offset,
            sample_increment: sample_increment,
            fade_ramp: RampedValue::new(1.0),
        }
    }

    /// Tick returns the delay position and the window gain
    pub fn tick(&mut self) -> (f32, f32) {
        if self.is_finished() {
            return (0.0, 0.0);
        }

        if self.is_waiting() {
            self.scheduled_wait = self.scheduled_wait - 1;
            return (0.0, 0.0);
        }

        if self.elapsed_sample_count == 0 && self.scheduled_wait == 0 {
            self.fade_ramp.set(0.0);
            self.fade_ramp.ramp(1.0, self.fade_duration);
        } else if self.elapsed_sample_count == (self.duration - self.fade_duration) {
            self.fade_ramp.set(1.0);
            self.fade_ramp.ramp(0.0, self.fade_duration);
        }

        let return_delay = self.delay_pos;
        self.delay_pos = self.delay_pos - self.sample_increment;
        self.elapsed_sample_count = self.elapsed_sample_count + 1;

        let win = self.fade_ramp.tick();
        (return_delay, win)
    }

    pub fn stop(&mut self) {
        // if already fading out don't stop it
        if self.elapsed_sample_count > (self.duration - self.fade_duration) {
            return;
        }

        // otherwise tweak the values so that the grain fades now
        self.duration = self.elapsed_sample_count + self.fade_duration;
    }

    pub fn is_finished(&self) -> bool {
        return self.elapsed_sample_count == self.duration || self.duration == 0;
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
    pub fn offset(&self) -> f32 {
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
        let mut grain = Grain::new(0, 10.0, 5, 0, false, 1.0);

        let expected = vec![
            (9.0, 1.0),
            (8.0, 1.0),
            (7.0, 1.0),
            (6.0, 1.0),
            (5.0, 1.0),
            (0.0, 0.0),
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
    fn test_grain_wait() {
        let mut grain = Grain::new(1, 10.0, 5, 0, false, 1.0);

        let expected = vec![
            (0.0, 0.0),
            (9.0, 1.0),
            (8.0, 1.0),
            (7.0, 1.0),
            (6.0, 1.0),
            (5.0, 1.0),
            (0.0, 0.0),
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
        let mut grain = Grain::new(0, 10.0, 9, 3, false, 1.0);

        let expected = vec![
            (9.0, 0.25),
            (8.0, 0.5),
            (7.0, 0.75),
            (6.0, 1.0),
            (5.0, 1.0),
            (4.0, 1.0),
            (3.0, 0.75),
            (2.0, 0.5),
            (1.0, 0.25),
            (0.0, 0.0),
        ];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
    }

    #[test]
    fn test_grain_stop() {
        let mut grain = Grain::new(0, 20.0, 15, 3, false, 1.0);

        let expected = vec![
            (19.0, 0.25),
            (18.0, 0.5),
            (17.0, 0.75),
            (16.0, 1.0),
            (15.0, 1.0),
        ];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);

        // stopping the grain should fade it out
        grain.stop();
        let expected_fade = vec![
            (14.0, 0.75),
            (13.0, 0.5),
            (12.0, 0.25),
            (0.0, 0.0),
            (0.0, 0.0),
        ];

        let mut out = vec![];
        for _i in 0..expected_fade.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected_fade);
        assert!(grain.is_finished());
    }

    #[test]
    fn test_grain_reverse() {
        let mut grain = Grain::new(0, 10.0, 5, 0, true, 1.0);

        let expected = vec![(5.0, 1.0), (6.0, 1.0), (7.0, 1.0), (8.0, 1.0), (9.0, 1.0)];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
        assert!(grain.is_finished());

        // check that normal grain is reverse of it
        let mut grain = Grain::new(0, 10.0, 5, 0, false, 1.0);
        let mut out_fwd = vec![];
        for _i in 0..expected.len() {
            out_fwd.push(grain.tick());
        }
        out_fwd.reverse();
        assert_eq!(out, out_fwd);
    }

    #[test]
    fn test_grain_fade_reverse() {
        let mut grain = Grain::new(0, 10.0, 10, 3, true, 1.0);

        let expected = vec![
            (0.0, 0.25),
            (1.0, 0.5),
            (2.0, 0.75),
            (3.0, 1.0),
            (4.0, 1.0),
            (5.0, 1.0),
            (6.0, 1.0),
            (7.0, 0.75),
            (8.0, 0.5),
            (9.0, 0.25),
            (0.0, 0.0),
        ];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
    }

    #[test]
    fn test_grain_half_speed() {
        let mut grain = Grain::new(0, 10.0, 5, 0, false, 0.5);

        let expected = vec![
            (9.0, 1.0),
            (8.5, 1.0),
            (8.0, 1.0),
            (7.5, 1.0),
            (7.0, 1.0),
            (0.0, 0.0),
        ];
        let mut out = vec![];
        for _i in 0..expected.len() {
            // assert!(!grain.is_finished());
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
        assert!(grain.is_finished());
    }
}
