use crate::delay_line::DelayLine;
use crate::grain_player::GrainPlayer;
use crate::ramped_value::RampedValue;

// how much of the buffer we allow to scrub through
const LOOPABLE_REGION_LENGTH: usize = 100000;
const DELAY_LINE_LENGTH: usize = 2 * LOOPABLE_REGION_LENGTH;

// uses a grain player to create loops
// owns two delay lines, one continously being
// written to by the input, one that is outputting loop
// when a new loop is started, the output delay line is
// copied to the input delay line
struct GrainLooper {
    grain_player: GrainPlayer,
    is_looping: bool,
    sample_rate: f32,
    ticks_till_next_loop: usize,
    // this is the buffer that is always being written to
    rolling_buffer: DelayLine,
    // this is the buffer that is only written to when looping, and when
    //the loopable region goes out of scope of the rolling buffer we switch to this one
    static_buffer: DelayLine,

    // ticks up as the rolling buffer scrolls left
    rolling_offset: usize,
    use_static_buffer: bool,
    loopable_region_length: usize,
    loop_offset: usize,
    loop_duration: usize,
    fade_duration: usize,
    dry_ramp: RampedValue,
}

#[allow(dead_code)]
impl GrainLooper {
    pub fn new(sample_rate: f32) -> GrainLooper {
        GrainLooper::new_with_length(sample_rate, DELAY_LINE_LENGTH)
    }

    fn new_with_length(sample_rate: f32, delay_line_length: usize) -> GrainLooper {
        let delay_line_rolling = DelayLine::new(delay_line_length);
        let delay_line_static = DelayLine::new(delay_line_length);
        GrainLooper {
            grain_player: GrainPlayer::new(),
            is_looping: false,
            sample_rate,
            ticks_till_next_loop: std::usize::MAX,
            rolling_buffer: delay_line_rolling,
            static_buffer: delay_line_static,
            rolling_offset: 0,
            use_static_buffer: false,
            loopable_region_length: delay_line_length / 2,
            loop_offset: 0,
            loop_duration: 0,
            fade_duration: 0,
            dry_ramp: RampedValue::new(1.0),
        }
    }

    pub fn set_fade_time(&mut self, fade: f32) {
        let fade_samples = (fade * self.sample_rate) as usize;
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

        // the loop duration should not be longer than the loopable region
        assert!(self.loop_duration < self.loopable_region_length);
    }

    pub fn start_looping(&mut self, loop_start_time_seconds: f32) {
        self.is_looping = true;
        // swap the buffers

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

    pub fn tick(&mut self, input: f32) -> f32 {
        self.rolling_buffer.tick(input);
        let looped;

        let dry = input;

        self.tick_next_loop_trigger();
        self.tick_static_buffer_copy();

        if self.use_static_buffer {
            looped = self.grain_player.tick(&self.static_buffer, 0);
        } else {
            looped = self
                .grain_player
                .tick(&self.rolling_buffer, self.rolling_offset);
        }

        let dry_level = self.dry_ramp.tick();
        looped + dry_level * dry
    }

    // this fills the static buffer with a copy of the rolling buffer, so
    // that when the loopable region exits the rolling buffer, we can use the static one
    fn tick_static_buffer_copy(&mut self) {
        // don't tick it if its full and we're using it
        if self.use_static_buffer {
            return;
        }
        // fill the static buffer with the loop region
        // we do this by reading the rolling buffer at a delay of the loopable region
        self.static_buffer
            .tick(self.rolling_buffer.read(self.loopable_region_length));
        self.rolling_offset += 1;

        // when the rolling offset has reached the end of the loopable region
        // we switch to the static buffer
        if self.rolling_offset >= self.loopable_region_length {
            self.use_static_buffer = true;
        }
    }

    fn tick_next_loop_trigger(&mut self) {
        if self.ticks_till_next_loop == 0 {
            self.grain_player.schedule_grain(
                0,
                self.loop_offset + self.fade_duration,
                self.loop_duration + self.fade_duration,
            );
            self.ticks_till_next_loop = self.loop_duration;
        } else {
            self.ticks_till_next_loop -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    fn all_near(a: &Vec<f32>, b: &Vec<f32>, epsilon: f32) {
        for i in 0..a.len() {
            assert_abs_diff_eq!(a[i], b[i], epsilon = epsilon);
        }
    }

    #[test]
    fn test_grain_looper_dry() {
        let mut looper = GrainLooper::new_with_length(10.0, 20);
        let mut out = vec![];
        for i in 0..5 {
            out.push(looper.tick(i as f32));
        }
        assert_eq!(out, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_grain_looper_loop() {
        // test a 5 sample loop, no fading, not using static buffer yet
        let mut looper = GrainLooper::new_with_length(10.0, 20);
        let mut out = vec![];
        for i in 0..5 {
            out.push(looper.tick(i as f32));
        }

        looper.set_fade_time(0.0);

        // set offset to be the loop length to loop the most recent 5 samples
        looper.set_loop_offset(0.5);
        looper.set_loop_duration(0.5);
        looper.start_looping(0.0);
        for i in 5..15 {
            out.push(looper.tick(i as f32));
        }
        assert_eq!(
            out,
            vec![0.0, 1.0, 2.0, 3.0, 4.0, 0.0, 1.0, 2.0, 3.0, 4.0, 0.0, 1.0, 2.0, 3.0, 4.0]
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
        let mut looper = GrainLooper::new_with_length(10.0, 50);
        let mut out = vec![];
        for i in 0..8 {
            out.push(looper.tick(1.0));
        }
        // start looping immediately
        // two samples fade
        looper.set_fade_time(0.2);
        // set offset to be the loop length to loop the most recent 5 samples
        looper.set_loop_offset(0.5);
        looper.set_loop_duration(0.5);
        looper.start_looping(0.0);
        for i in 8..15 {
            out.push(looper.tick(1.0));
        }

        assert_eq!(
            out,
            vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );

        // stop looping
        out.clear();
        looper.stop_looping();
        for i in 15..20 {
            out.push(looper.tick(1.0));
        }

        // expect the dry to fade back in from whatever the loop was doing
        assert_eq!(out, vec![1.0, 1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_grain_looper_fade() {
        let mut looper = GrainLooper::new_with_length(10.0, 50);
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
        // first loop is same as dry
        // but the next will fade between 0,1,2,3,4,5,6,7,8       4,5,6,7,8,9
        //                                   |       |       |       |
        // and                                        0,1,2,3,4,5,
        let first_faded = 6.0 * 2.0 / 3.0;
        let second_faded = 7.0 / 3.0 + 2.0 / 3.0;
        let third_faded = 4.0 * 2.0 / 3.0;
        let fourth_faded = 5.0 / 3.0 + 2.0 / 3.0;
        all_near(
            &out,
            &vec![
                0.0,
                1.0,
                2.0,
                3.0,
                4.0,
                5.0,
                first_faded,
                second_faded,
                2.0,
                3.0,
                third_faded,
                fourth_faded,
                2.0,
                3.0,
                third_faded,
                fourth_faded,
            ],
            0.0001,
        );

        out.clear();
        looper.stop_looping();
        for i in 15..20 {
            out.push(looper.tick(i as f32));
        }

        // // expect the dry to fade back in from whatever the loop was doing
        // all_near(&out, &vec![23.0, 23.0, 22.0, 23.0, 24.0], 0.0001);
    }
}
