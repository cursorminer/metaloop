use crate::delay_line::DelayLine;
use crate::grain::Grain;
use crate::grain_player::GrainPlayer;
use crate::ramped_value::RampedValue;

// how much of the buffer we allow to scrub through
// TODO set these to be seconds
const LOOPABLE_REGION_LENGTH: usize = 100000;
const MAX_FADE_TIME: usize = 1000;

// uses a grain player to create loops
// owns two delay lines, one continously being
// written to by the input, one that is outputting loop
// when a new loop is started, the output delay line is
// copied to the input delay line
pub struct GrainLooper {
    grain_player: GrainPlayer<f32>,
    is_looping: bool,
    sample_rate: f32,
    ticks_till_next_loop: usize,

    loop_offset: usize,
    loop_duration: usize,
    fade_duration: usize,
    dry_ramp: RampedValue,
    ticks_since_loop_start: usize,
    reverse: bool,
    speed: f32,
}

// Loops segments of audio, with the ability to scrub through the loop
// sets loop offset and duration in seconds
#[allow(dead_code)]
impl GrainLooper {
    pub fn new(sample_rate: f32) -> GrainLooper {
        GrainLooper::new_with_length(sample_rate, LOOPABLE_REGION_LENGTH, MAX_FADE_TIME)
    }

    fn new_with_length(
        sample_rate: f32,
        loopable_region_length: usize,
        max_fade_time: usize,
    ) -> GrainLooper {
        GrainLooper {
            grain_player: GrainPlayer::new_with_length(
                sample_rate,
                loopable_region_length,
                max_fade_time,
            ),
            is_looping: false,
            sample_rate,
            ticks_till_next_loop: std::usize::MAX,

            loop_offset: 0,
            loop_duration: 0,
            fade_duration: 0,

            dry_ramp: RampedValue::new(1.0),
            ticks_since_loop_start: 0,
            reverse: false,
            speed: 1.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn set_fade_time(&mut self, fade: f32) {
        let fade_samples = (fade * self.sample_rate) as usize;
        assert!(fade_samples <= MAX_FADE_TIME);

        self.fade_duration = fade_samples;
    }

    // offset the loop in the buffer, i.e. "scrub"
    pub fn set_loop_offset(&mut self, offset_seconds: f32) {
        self.loop_offset = (offset_seconds * self.sample_rate) as usize;
    }

    // how long the loop is
    pub fn set_loop_duration(&mut self, duration_seconds: f32) {
        if !self.is_looping || self.ticks_since_loop_start > self.loop_duration {
            self.loop_duration = (duration_seconds * self.sample_rate) as usize;
            return;
        }

        // check to see if we have enough buffer to loop
        // if the number of ticks since lop start is greater than the loop duration
        // then it is ok to change the loop

        // other wise, we need to fade back to dry and wait for the buffer to contain the whole loop
        self.dry_ramp.ramp(1.0, self.fade_duration);
        self.grain_player.stop_all_grains();

        let wait = self.loop_duration - self.ticks_since_loop_start;
        self.schedule_grain(wait);
        self.ticks_till_next_loop = wait + self.loop_duration;

        // the loop duration and fade should not be longer than the loopable region
        assert!(
            self.loop_duration + self.fade_duration <= self.grain_player.loopable_region_length()
        );
    }

    // note that the loop_start_point_seconds is toward the past, as we want to loop something that has already started
    pub fn start_looping(&mut self, loop_start_point_seconds: f32) {
        self.is_looping = true;

        // how far are we past the loop start time
        self.ticks_since_loop_start = (loop_start_point_seconds * self.sample_rate) as usize;

        // schedule the first grain
        let wait = self.loop_duration - self.ticks_since_loop_start;

        // offset needs to have the fade before it, so that the transient is at full vol
        // duration needs to have the fade after it, as the fading region is at the end
        self.schedule_grain(wait);

        self.ticks_till_next_loop = wait + self.loop_duration;
        self.dry_ramp.set(1.0);
        self.dry_ramp.ramp(0.0, self.fade_duration);
    }

    fn schedule_grain(&mut self, wait: usize) {
        self.grain_player.schedule_grain(Grain::new(
            wait,
            (self.loop_offset + self.fade_duration) as f32,
            self.loop_duration + self.fade_duration,
            self.fade_duration,
            self.reverse,
            self.speed,
        ));
    }

    pub fn stop_looping(&mut self) {
        self.is_looping = false;
        self.ticks_till_next_loop = std::usize::MAX;
        // start a fade back to dry
        self.grain_player.stop_all_grains();
        self.dry_ramp.set(0.0);
        self.dry_ramp.ramp(1.0, self.fade_duration);
    }

    pub fn set_reverse(&mut self, reverse: bool) {
        self.reverse = reverse;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn tick(&mut self, input: f32) -> f32 {
        self.ticks_since_loop_start += 1;

        let dry = input;

        self.tick_next_loop_trigger();

        let looped = self.grain_player.tick(input);

        let dry_level = self.dry_ramp.tick();
        looped + dry_level * dry
    }

    // ticks down the counter that will trigger a new loop as the old one starts to fade out
    fn tick_next_loop_trigger(&mut self) {
        if self.ticks_till_next_loop == 0 {
            self.schedule_grain(0);
            self.ticks_till_next_loop = self.loop_duration;
        }
        self.ticks_till_next_loop -= 1;
    }

    fn num_playing_grains(&self) -> usize {
        self.grain_player.num_playing_grains()
    }

    pub fn is_looping(&self) -> bool {
        self.is_looping
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    use approx::assert_abs_diff_eq;

    fn all_near(a: &Vec<f32>, b: &Vec<f32>, epsilon: f32) {
        for i in 0..a.len() {
            assert_abs_diff_eq!(a[i], b[i], epsilon = epsilon);
        }
    }

    #[test]
    fn test_grain_looper_dry() {
        let mut looper = GrainLooper::new_with_length(10.0, 20, 0);
        let mut out = vec![];
        for i in 0..5 {
            out.push(looper.tick(i as f32));
        }
        assert_eq!(out, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_grain_looper_loop() {
        // test a 5 sample loop, no fading, not using static buffer yet
        let mut looper = GrainLooper::new_with_length(10.0, 20, 0);
        let mut out = vec![];
        for i in 0..5 {
            out.push(looper.tick((i + 10) as f32));
        }

        looper.set_fade_time(0.0);

        // set offset to be the loop length to loop the most recent 5 samples
        looper.set_loop_offset(0.5);
        looper.set_loop_duration(0.5);
        looper.start_looping(0.5);
        for i in 5..20 {
            out.push(looper.tick((i + 10) as f32));
        }
        assert_eq!(
            out,
            vec![
                10.0, 11.0, 12.0, 13.0, 14.0, 10.0, 11.0, 12.0, 13.0, 14.0, 10.0, 11.0, 12.0, 13.0,
                14.0, 10.0, 11.0, 12.0, 13.0, 14.0
            ]
        );

        out.clear();
        // stop looping
        looper.stop_looping();
        for i in 15..20 {
            out.push(looper.tick(i as f32));
        }
        // back to dry
        assert_eq!(out, vec![15.0, 16.0, 17.0, 18.0, 19.0]);
    }

    #[test]
    fn test_grain_looper_fade_is_flat() {
        // when we loop a DC signal we expect the fades to maintain the DC level
        let mut looper = GrainLooper::new_with_length(10.0, 50, 4);
        let mut out = vec![];
        for _i in 0..8 {
            out.push(looper.tick(1.0));
        }
        // start looping immediately
        // two samples fade
        looper.set_fade_time(0.2);
        // set offset to be the loop length to loop the most recent 5 samples
        looper.set_loop_offset(0.5);
        looper.set_loop_duration(0.5);
        looper.start_looping(0.5);
        for _i in 8..15 {
            out.push(looper.tick(1.0));
        }

        assert_eq!(
            out,
            vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );

        // stop looping
        out.clear();
        looper.stop_looping();
        for _i in 15..20 {
            out.push(looper.tick(1.0));
        }

        // expect the dry to fade back in from whatever the loop was doing
        assert_eq!(out, vec![1.0, 1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_grain_looper_fade() {
        let mut looper = GrainLooper::new_with_length(10.0, 50, 4);
        let mut out = vec![];

        let loop_start = 6;
        let loop_stop = 12;

        for i in 0..loop_start {
            out.push(looper.tick(i as f32));
        }
        // start looping immediately
        // two samples fade
        looper.set_fade_time(0.2);
        // set offset to be the loop length to loop the most recent 4 samples
        looper.set_loop_offset(0.4);
        looper.set_loop_duration(0.4);
        looper.start_looping(0.4);

        for i in loop_start..loop_stop {
            out.push(looper.tick(i as f32));
        }

        let first_faded = 6.0 * 2.0 / 3.0;
        let second_faded = 7.0 / 3.0 + 2.0 / 3.0;

        let third = 0.33333334;
        let two_thirds = 0.6666667;
        let loop_grain_contents = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let fades = vec![third, two_thirds, 1.0, 1.0, two_thirds, third];
        let faded = loop_grain_contents
            .iter()
            .zip(fades.iter())
            .map(|(a, b)| a * b)
            .collect::<Vec<f32>>();
        let mut loop_overlapped = vec![];
        for i in 0..4 {
            let overlapped = faded.get(i + 4).unwrap_or(&0.0);
            loop_overlapped.push(faded[i] + overlapped);
        }
        loop_overlapped.rotate_left(2);

        let mut expected = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, first_faded, second_faded];

        expected.extend(&loop_overlapped);
        expected.extend(&loop_overlapped);
        expected.extend(&loop_overlapped);

        all_near(&out, &expected, 0.0001);

        out.clear();
        looper.stop_looping();
        for i in 15..20 {
            out.push(looper.tick(i as f32));
        }

        // expect the dry to fade back in from whatever the loop was doing
        let fifth_fade = 15.0 / 3.0 + 2.0 * 2.0 / 3.0;
        let sixth_fade = 16.0 * 2.0 / 3.0 + 3.0 / 3.0;
        all_near(
            &out,
            &vec![fifth_fade, sixth_fade, 17.0, 18.0, 19.0],
            0.0001,
        );
    }

    #[test]
    fn test_grain_looper_tweak_loop() {
        // test that we can change the offset and length of the loop
        let mut looper = GrainLooper::new_with_length(10.0, 50, 0);
        let mut out = vec![];

        let loop_start_at = 8;
        let change_offset_at = 14;
        let stop_at = 25;

        for i in 0..loop_start_at {
            out.push(looper.tick(i as f32));
        }

        looper.set_fade_time(0.0);
        // set offset to be the loop length to loop the most recent 4 samples (4,5,6,7)
        looper.set_loop_offset(0.4);
        looper.set_loop_duration(0.4);
        looper.start_looping(0.4);

        for i in loop_start_at..change_offset_at {
            out.push(looper.tick(i as f32));
        }
        // offset the loop backwards by 2 samples (2,3,4,5)
        looper.set_loop_offset(0.6);
        // change the length of the loop to be 3 samples (2,3,4)
        looper.set_loop_duration(0.3);
        // reverse the loop (4,3,2)
        looper.set_reverse(true);
        // slow the loop down by half (4, 3.5, 3)
        looper.set_speed(0.5);

        for i in change_offset_at..stop_at {
            out.push(looper.tick(i as f32));
        }

        let mut expected = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

        let first_loop = vec![4.0, 5.0, 6.0, 7.0];
        let second_loop = vec![4.0, 3.5, 3.0];

        expected.extend(&first_loop);
        expected.extend(&first_loop);
        expected.extend(&second_loop);
        expected.extend(&second_loop);
        expected.extend(&second_loop);

        assert_eq!(out, expected);
    }

    #[test]
    fn test_grain_looper_immediate_reverse_without_fade() {
        // test that an immediate reverse with a fade does not try to read into the future
        let mut looper = GrainLooper::new_with_length(10.0, 50, 0);
        let mut out = vec![];

        let loop_start_at = 8;
        let stop_at = 16;

        for i in 0..loop_start_at {
            out.push(looper.tick(i as f32));
        }

        looper.set_fade_time(0.0);
        // set offset to be the loop length to loop the most recent 4 samples (4,5,6,7)
        looper.set_loop_offset(0.4);
        looper.set_loop_duration(0.4);
        looper.set_reverse(true);
        looper.start_looping(0.4);

        for i in loop_start_at..stop_at {
            out.push(looper.tick(i as f32));
        }

        let mut expected = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

        let loop_samples = vec![7.0, 6.0, 5.0, 4.0];

        expected.extend(&loop_samples);
        expected.extend(&loop_samples);

        assert_eq!(out, expected);
    }

    #[test]
    fn test_grain_looper_immediate_reverse_with_fade() {
        // test that an immediate reverse with a fade does not try to read into the future
        let mut looper = GrainLooper::new_with_length(10.0, 50, 4);
        let mut out = vec![];

        let loop_start_at = 8;
        let stop_at = 18;

        for i in 0..loop_start_at {
            out.push(looper.tick(i as f32));
        }

        looper.set_fade_time(0.2);
        // set offset to be the loop length to loop the most recent 4 samples (4,5,6,7)
        looper.set_loop_offset(0.4);
        looper.set_loop_duration(0.4);
        looper.set_reverse(true);
        looper.start_looping(0.4);

        for i in loop_start_at..stop_at {
            out.push(looper.tick(i as f32));
        }

        let mut expected = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let initial_fade = vec![7.666667, 7.0];

        let loop_samples = vec![5.0, 4.0, 4.333333, 4.6666665];

        expected.extend(&initial_fade);
        expected.extend(&loop_samples);
        expected.extend(&loop_samples);
        assert_eq!(out, expected);
        all_near(&out, &expected, 0.0001);
    }

    #[test]
    fn test_grain_looper_short_to_long() {
        // test a 5 sample loop, no fading
        let mut looper = GrainLooper::new_with_length(10.0, 20, 0);
        let mut out = vec![];

        let loop_start = 5;
        let loop_change = 10;
        let loop_stop = 26;

        let first_len = 0.2;
        let second_len = 0.8;

        for i in 0..loop_start {
            out.push(looper.tick((i + 10) as f32));
        }

        looper.set_fade_time(0.0);

        // set offset to be the loop length to loop the most recent 5 samples
        looper.set_loop_offset(first_len);
        looper.set_loop_duration(first_len);
        looper.start_looping(first_len);
        for i in loop_start..loop_change {
            out.push(looper.tick((i + 10) as f32));
        }

        looper.set_loop_duration(second_len);

        // this means that the loop is 8 samples long but we don't have enough samples to do it yet, so we switch back to dry
        // we wait till the last loop stops
        for i in loop_change..loop_stop {
            out.push(looper.tick((i + 10) as f32));
        }

        let mut expected = vec![10.0, 11.0, 12.0, 13.0, 14.0];
        let loop_one = vec![13.0, 14.0];
        // when loop one stops and loop two is yet to start, should resemble end of loop two
        let dry_thru = vec![17.0, 18.0, 19.0, 20.0];
        let loop_two = vec![13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0];

        expected.extend(&loop_one);
        expected.extend(&loop_one);
        expected.extend(&loop_one);
        expected.extend(&dry_thru);
        expected.extend(&loop_two);
        expected.extend(&loop_two);

        assert_eq!(out, expected);
    }
}
