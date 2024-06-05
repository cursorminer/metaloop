use crate::delay_line::DelayLine;
use crate::grain::Grain;
use crate::grain_player::GrainPlayer;
use crate::loop_scheduler::LoopEvent;
use crate::loop_scheduler::LoopScheduler;
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
    loop_scheduler: LoopScheduler,
    is_looping: bool,
    sample_rate: f32,

    loop_offset: usize,
    fade_duration: usize,
    dry_ramp: RampedValue,
    song_ticks: usize,
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
            loop_scheduler: LoopScheduler::new(),
            is_looping: false,
            sample_rate,

            loop_offset: 0,
            fade_duration: 0,

            dry_ramp: RampedValue::new(1.0),
            song_ticks: 0,
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

        self.loop_scheduler.set_fade_lead_in(fade);
    }

    // offset the loop in the buffer, i.e. "scrub"
    pub fn set_loop_offset(&mut self, offset_seconds: f32) {
        self.loop_offset = (offset_seconds * self.sample_rate) as usize;
    }

    // how long the loop is
    pub fn set_grid(&mut self, duration_seconds: f32) {
        self.loop_scheduler.set_grid_interval(duration_seconds);
    }

    // note that the loop_start_point_seconds is toward the past, as we want to loop something that has already started
    pub fn start_looping(&mut self) {
        self.loop_scheduler.start_looping();
    }

    fn schedule_grain(&mut self, wait: usize, duration: usize) {
        // wait might go away
        self.grain_player.schedule_grain(Grain::new(
            wait,
            self.loop_offset as f32,
            duration + self.fade_duration,
            self.fade_duration,
            self.reverse,
            self.speed,
        ));
    }

    pub fn stop_looping(&mut self) {
        self.loop_scheduler.stop_looping();
    }

    pub fn set_reverse(&mut self, reverse: bool) {
        self.reverse = reverse;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn tick(&mut self, input: f32) -> f32 {
        // for now we work out the beat time here
        let song_time = self.song_ticks as f32 / self.sample_rate;
        let events = self.loop_scheduler.tick(song_time);
        println!("events: {:?}", events);

        for event in events {
            match event {
                LoopEvent::StartGrain { duration } => {
                    print!("start grain\n");
                    self.schedule_grain(0, self.beat_time__to_samples(duration));
                    self.is_looping = true;
                }
                LoopEvent::StopGrain => {
                    // we stop them all
                    print!("stop grain\n");
                    self.grain_player.stop_all_grains();
                }
                LoopEvent::FadeInDry => {
                    print!("fade in dry\n");
                    self.dry_ramp.ramp(1.0, self.fade_duration);
                }
                LoopEvent::FadeOutDry => {
                    print!("fade out dry\n");
                    self.dry_ramp.ramp(0.0, self.fade_duration);
                }
                _ => {}
            }
        }
        self.song_ticks += 1;

        let dry = input;

        let looped = self.grain_player.tick(input);
        println!("looped: {}", looped);

        let dry_level = self.dry_ramp.tick();
        println!("dry_level: {}", dry_level);
        looped + dry_level as f32 * dry
    }

    fn beat_time__to_samples(&self, time: f32) -> usize {
        // at thme moment its in seconds
        (time * self.sample_rate) as usize
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
        if a.len() != b.len() {
            println!("");
            println!("left = {:?}\nright = {:?}", a, b);
            println!("");
            panic!("lengths differ: {} != {}", a.len(), b.len());
        }
        let near = a
            .iter()
            .zip(b.iter())
            .map(|(a, b)| (a - b).abs())
            .all(|x| x < epsilon);
        println!("");
        assert!(near, "left = {:?}\nright = {:?}", a, b);
        println!("");
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
        looper.set_loop_offset(0.0);
        looper.set_grid(0.5);
        looper.start_looping();
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
        looper.set_loop_offset(0.0);
        looper.set_grid(0.5);
        looper.start_looping();
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

    fn overlap_fade(loop_grain_contents: Vec<f32>, fades: Vec<f32>) -> Vec<f32> {
        let faded_grain = loop_grain_contents
            .iter()
            .zip(fades.iter())
            .map(|(a, b)| a * b)
            .collect::<Vec<f32>>();
        let mut loop_overlapped = vec![];
        for i in 0..4 {
            let overlapped = faded_grain.get(i + 4).unwrap_or(&0.0);
            loop_overlapped.push(faded_grain[i] + overlapped);
        }
        loop_overlapped.rotate_left(2);
        loop_overlapped
    }

    #[test]
    fn test_grain_looper_fade() {
        // test that a single loop fades into the next loop
        let mut looper = GrainLooper::new_with_length(10.0, 50, 4);
        let mut out = vec![];

        let loop_start = 6;
        let loop_stop = 20;

        let one_third = 0.33333334;
        let two_third = 0.6666667;

        for i in 0..loop_start {
            out.push(looper.tick((i + 10) as f32));
        }

        // two samples fade
        looper.set_fade_time(0.2);

        // set offset to be the loop length to loop the most recent 4 samples
        looper.set_loop_offset(0.0);
        // loop len 4
        looper.set_grid(0.4);

        let dry_fade_1 = 16.0 * two_third;
        let dry_fade_2 = 17.0 * one_third;

        let loop_grain_contents = vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0];
        let fades = vec![one_third, two_third, 1.0, 1.0, two_third, one_third];

        let loop_overlapped = overlap_fade(loop_grain_contents.clone(), fades.clone());

        // println!("loop_overlapped: {:?}", loop_overlapped);
        // 12.0, 13.0, 12.666668, 12.333334

        let mut expected = vec![
            10.0,
            11.0,
            12.0,
            13.0,
            14.0,
            15.0,
            dry_fade_1 + loop_grain_contents[0] * fades[0],
            dry_fade_2 + loop_grain_contents[1] * fades[1],
        ];

        expected.extend(&loop_overlapped);
        expected.extend(&loop_overlapped);
        expected.extend(&loop_overlapped);

        looper.start_looping();

        for i in loop_start..loop_stop {
            out.push(looper.tick((i + 10) as f32));
        }

        all_near(&out, &expected, 0.0001);

        out.clear();
        looper.stop_looping();

        // expect the loop to finish, and then the dry to fade back in from whatever the loop was doing

        let finish_test = loop_stop + 6;
        for i in loop_stop..finish_test {
            out.push(looper.tick((i + 10) as f32));
        }

        all_near(&out, &vec![12.0, 13.0, 20.0, 27.0, 34.0, 35.0], 0.0001);
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
        looper.set_loop_offset(0.1);
        looper.set_grid(0.4);
        looper.start_looping();

        for i in loop_start_at..change_offset_at {
            out.push(looper.tick(i as f32));
        }
        // offset the loop backwards by 2 samples (2,3,4,5)
        looper.set_loop_offset(0.2);
        // change the length of the loop to be 3 samples (2,3,4)
        looper.set_grid(0.3);
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
        looper.set_grid(0.4);
        looper.set_reverse(true);
        looper.start_looping();

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
        looper.set_grid(0.4);
        looper.set_reverse(true);
        looper.start_looping();

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
        // test that if a short loop is changed to a longer loop, it still starts in the same place
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
        looper.set_grid(first_len);
        looper.start_looping();
        for i in loop_start..loop_change {
            out.push(looper.tick((i + 10) as f32));
        }

        looper.set_grid(second_len);

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
