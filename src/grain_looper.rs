use crate::delay_line::DelayLine;
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
    grain_player: GrainPlayer,
    is_looping: bool,
    sample_rate: f32,
    ticks_till_next_loop: usize,
    // this is the buffer that is always being written to
    rolling_buffer: DelayLine<f32>,
    // this is the buffer that is only written to when looping, and when
    //the loopable region goes out of scope of the rolling buffer we switch to this one
    static_buffer: DelayLine<f32>,

    // ticks up as the rolling buffer scrolls left
    rolling_offset: usize,
    use_static_buffer: bool,
    loopable_region_length: usize,
    loop_offset: usize,
    loop_duration: usize,
    fade_duration: usize,
    fade_allowance: usize,
    dry_ramp: RampedValue,
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
        let delay_line_length_rolling = loopable_region_length * 2 + max_fade_time;
        let delay_line_length_static = loopable_region_length + max_fade_time;
        let delay_line_rolling = DelayLine::new(delay_line_length_rolling);
        let delay_line_static = DelayLine::new(delay_line_length_static);
        GrainLooper {
            grain_player: GrainPlayer::new(),
            is_looping: false,
            sample_rate,
            ticks_till_next_loop: std::usize::MAX,
            rolling_buffer: delay_line_rolling,
            static_buffer: delay_line_static,
            rolling_offset: 0,
            use_static_buffer: false,
            loopable_region_length: loopable_region_length,
            loop_offset: 0,
            loop_duration: 0,
            fade_duration: 0,
            fade_allowance: max_fade_time,
            dry_ramp: RampedValue::new(1.0),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn set_fade_time(&mut self, fade: f32) {
        let fade_samples = (fade * self.sample_rate) as usize;
        assert!(fade_samples <= self.fade_allowance);
        self.grain_player.set_fade_time(fade_samples);
        self.fade_duration = fade_samples;
    }

    // offset the loop in the buffer, i.e. "scrub"
    pub fn set_loop_offset(&mut self, offset_seconds: f32) {
        self.loop_offset = (offset_seconds * self.sample_rate) as usize;
    }

    // how long the loop is
    pub fn set_loop_duration(&mut self, duration_seconds: f32) {
        self.loop_duration = (duration_seconds * self.sample_rate) as usize;

        // the loop duration and fade should not be longer than the loopable region
        assert!(self.loop_duration + self.fade_duration <= self.loopable_region_length);
    }

    // note that the loop start time should be offset by the fade duration in order to sync the maximum of the fade
    // with the transient at the start of the loop
    pub fn start_looping(&mut self, loop_start_time_seconds: f32) {
        self.is_looping = true;

        // schedule the first grain
        let wait = (loop_start_time_seconds * self.sample_rate) as usize;

        // offset needs to have the fade before it, so that the transient is at full vol
        // duration needs to have the fade after it, as the fading region is at the end
        self.grain_player.schedule_grain(
            wait,
            self.loop_offset + self.fade_duration,
            self.loop_duration + self.fade_duration,
        );

        self.ticks_till_next_loop = wait + self.loop_duration;
        self.rolling_offset = 0;
        self.use_static_buffer = false;
        self.dry_ramp.set(1.0);
        self.dry_ramp.ramp(0.0, self.fade_duration);
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
        self.grain_player.set_reverse(reverse);
    }

    pub fn tick(&mut self, input: f32) -> f32 {
        self.rolling_buffer.tick(input);
        self.rolling_offset += 1;
        let looped;

        let dry = input;

        self.tick_next_loop_trigger();

        if self.use_static_buffer {
            looped = self
                .grain_player
                .tick(&self.static_buffer, self.fade_allowance);
        } else {
            looped = self
                .grain_player
                .tick(&self.rolling_buffer, self.rolling_offset);
        }
        self.tick_static_buffer_copy();

        let dry_level = self.dry_ramp.tick();
        looped + dry_level * dry
    }

    // this fills the static buffer with a copy of the rolling buffer, so
    // that when the loopable region exits the rolling buffer, we can use the static one
    fn tick_static_buffer_copy(&mut self) {
        // don't tick it if its full and we're using it, or if we're not looping
        if self.use_static_buffer || !self.is_looping {
            return;
        }
        // fill the static buffer with the loop region
        // we do this by reading the rolling buffer at a delay of the loopable region
        self.static_buffer
            .tick(self.rolling_buffer.read(self.loopable_region_length));

        // when the rolling offset has reached the end of the loopable region, and the fade allowance
        // we switch to the static buffer
        if self.rolling_offset >= self.ticks_before_switch_to_static_buffer() {
            self.use_static_buffer = true;
        }
    }

    // ticks down the counter that will trigger a new loop as the old one starts to fade out
    fn tick_next_loop_trigger(&mut self) {
        if self.ticks_till_next_loop == 0 {
            self.grain_player.schedule_grain(
                0,
                self.loop_offset + self.fade_duration,
                self.loop_duration + self.fade_duration,
            );
            self.ticks_till_next_loop = self.loop_duration;
        }
        self.ticks_till_next_loop -= 1;
    }

    fn ticks_before_switch_to_static_buffer(&self) -> usize {
        self.loopable_region_length + self.fade_allowance
    }

    fn is_using_static_buffer(&self) -> bool {
        self.use_static_buffer
    }

    fn static_buffer(&self) -> &DelayLine<f32> {
        &self.static_buffer
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
        looper.start_looping(0.0);
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
        looper.start_looping(0.0);
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
        looper.start_looping(0.0);

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
    fn test_grain_looper_switch_to_static() {
        // the loopable region is only 4 samples long
        let mut looper = GrainLooper::new_with_length(8.0, 4, 0);
        let mut out = vec![];

        let loop_start = 4;
        let ticks_before_switch_to_static_buffer = looper.ticks_before_switch_to_static_buffer();

        // fill the rolling buffer with 4 samples
        for i in 0..loop_start {
            out.push(looper.tick((i + 10) as f32));
        }
        // start looping those 4 samples
        looper.set_fade_time(0.0);
        looper.set_loop_offset(0.5);
        looper.set_loop_duration(0.5);
        looper.start_looping(0.0);

        assert!(!looper.is_using_static_buffer());

        // now the static buffer starts filling
        for i in loop_start..loop_start + ticks_before_switch_to_static_buffer {
            out.push(looper.tick((i + 10) as f32));
        }
        // after the loopable region has passed, we should be using the static buffer
        assert!(looper.is_using_static_buffer());

        // check the output loops as normal
        for i in loop_start + ticks_before_switch_to_static_buffer..20 {
            out.push(looper.tick((i + 10) as f32));
        }
        // expect the contents of static buffer to be  the first 4 samples
        let expected_static = vec![13.0, 12.0, 11.0, 10.0];
        let static_buffer = looper.static_buffer();
        for i in 0..4 {
            assert_eq!(static_buffer.read(i), expected_static[i]);
        }
        assert_eq!(
            out,
            vec![
                10.0, 11.0, 12.0, 13.0, 10.0, 11.0, 12.0, 13.0, 10.0, 11.0, 12.0, 13.0, 10.0, 11.0,
                12.0, 13.0, 10.0, 11.0, 12.0, 13.0
            ]
        );
    }

    #[test]
    fn test_grain_looper_switch_to_static_fade() {
        // like above but with a tricky thing that the fade needs to be appended to the static buffer
        let mut looper = GrainLooper::new_with_length(8.0, 8, 2);
        let mut out = vec![];

        let loop_start = 6;
        let ticks_before_switch_to_static_buffer = looper.ticks_before_switch_to_static_buffer();

        // fill the rolling buffer with 4 samples
        for i in 0..loop_start {
            out.push(looper.tick((i + 10) as f32));
        }
        // set fade to two samples
        looper.set_fade_time(0.25);
        looper.set_loop_offset(0.5);
        looper.set_loop_duration(0.5);
        looper.start_looping(0.0);

        for i in loop_start..loop_start + ticks_before_switch_to_static_buffer {
            assert!(!looper.is_using_static_buffer());
            out.push(looper.tick((i + 10) as f32));
        }
        // after looping for the entire loopable region, the static buffer should be full
        assert!(looper.is_using_static_buffer());

        for i in ticks_before_switch_to_static_buffer
            ..loop_start + ticks_before_switch_to_static_buffer * 2
        {
            out.push(looper.tick((i + 10) as f32));
        }
        // expect the contents of static buffer to be the loop, plus 2 samples to fade
        let expected_static = vec![17.0, 16.0, 15.0, 14.0, 13.0, 12.0, 11.0, 10.0, 0.0, 0.0];
        let static_buffer = looper.static_buffer();
        for i in 0..looper.static_buffer().len() {
            assert_eq!(static_buffer.read(i), expected_static[i]);
        }

        let mut expected = vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 14.0, 13.0];
        let fade_1 = 14.0 * 2.0 / 3.0 + 10.0 / 3.0;
        let fade_2 = 15.0 / 3.0 + 11.0 * 2.0 / 3.0;
        let loop_overlapped = vec![12.0, 13.0, fade_1, fade_2];

        expected.extend(&loop_overlapped);
        expected.extend(&loop_overlapped);
        expected.extend(&loop_overlapped);
        expected.extend(&loop_overlapped);
        expected.extend(&loop_overlapped);
        expected.extend(&loop_overlapped);

        all_near(&out, &expected, 0.0001);
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
        looper.start_looping(0.0);

        for i in loop_start_at..change_offset_at {
            out.push(looper.tick(i as f32));
        }
        // offset the loop backwards by 2 samples (2,3,4,5)
        looper.set_loop_offset(0.6);
        // change the length of the loop to be 3 samples (2,3,4)
        looper.set_loop_duration(0.3);
        looper.set_reverse(true);

        for i in change_offset_at..stop_at {
            out.push(looper.tick(i as f32));
        }

        let mut expected = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

        let first_loop = vec![4.0, 5.0, 6.0, 7.0];
        let second_loop = vec![4.0, 3.0, 2.0];

        expected.extend(&first_loop);
        expected.extend(&first_loop);
        expected.extend(&second_loop);
        expected.extend(&second_loop);
        expected.extend(&second_loop);

        assert_eq!(out, expected);
    }

    fn test_grain_looper_immediate_reverse_without_fade() {
        // test that an immediate reverse with a fade does not try to read into the future
        let mut looper = GrainLooper::new_with_length(10.0, 50, 0);
        let mut out = vec![];

        let loop_start_at = 8;
        let stop_at = 14;

        for i in 0..loop_start_at {
            out.push(looper.tick(i as f32));
        }

        looper.set_fade_time(0.0);
        // set offset to be the loop length to loop the most recent 4 samples (4,5,6,7)
        looper.set_loop_offset(0.4);
        looper.set_loop_duration(0.4);
        looper.set_reverse(true);
        looper.start_looping(0.0);

        for i in loop_start_at..stop_at {
            out.push(looper.tick(i as f32));
        }
    }

    #[test]
    fn test_grain_looper_immediate_reverse_with_fade() {
        // test that an immediate reverse with a fade does not try to read into the future
        let mut looper = GrainLooper::new_with_length(10.0, 50, 4);
        let mut out = vec![];

        let loop_start_at = 8;
        let stop_at = 14;

        for i in 0..loop_start_at {
            out.push(looper.tick(i as f32));
        }

        looper.set_fade_time(0.2);
        // set offset to be the loop length to loop the most recent 4 samples (4,5,6,7)
        looper.set_loop_offset(0.4);
        looper.set_loop_duration(0.4);
        looper.set_reverse(true);
        looper.start_looping(0.0);

        for i in loop_start_at..stop_at {
            out.push(looper.tick(i as f32));
        }
    }
}
