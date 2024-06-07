use crate::grain::Grain;
use crate::grain_player::GrainPlayer;
use crate::loop_scheduler::LoopEvent;
use crate::loop_scheduler::LoopScheduler;
use crate::ramped_value::RampedValue;

// how much of the buffer we allow to scrub through
// TODO set these to be seconds
const LOOPABLE_REGION_LENGTH: usize = 100000;
const MAX_FADE_TIME: usize = 1000;
const MAX_LOOP_LENGTH: usize = LOOPABLE_REGION_LENGTH / 2;

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

    loop_offset_beats: f32,
    fade_duration: usize,
    dry_ramp: RampedValue,
    song_ticks: usize,
    reverse: bool,
    speed: f32,
    tempo: f32,
    beats_per_tick: f32,
}

pub fn seconds_to_beats(seconds: f32, tempo: f32) -> f32 {
    seconds * tempo / 60.0
}

pub fn beats_to_seconds(beats: f32, tempo: f32) -> f32 {
    beats * 60.0 / tempo
}

pub fn seconds_to_samples(seconds: f32, sample_rate: f32) -> usize {
    (seconds * sample_rate) as usize
}

pub fn samples_to_beats(samples: usize, tempo: f32, sample_rate: f32) -> f32 {
    samples as f32 / sample_rate * tempo / 60.0
}

pub fn beats_to_samples(beats: f32, tempo: f32, sample_rate: f32) -> usize {
    (beats * 60.0 / tempo * sample_rate) as usize
}

// Loops segments of audio, with the ability to scrub through the loop
// sets loop offset and duration in seconds
#[allow(dead_code)]
impl GrainLooper {
    pub fn new(sample_rate: f32) -> GrainLooper {
        GrainLooper::new_with_length(
            sample_rate,
            LOOPABLE_REGION_LENGTH,
            MAX_FADE_TIME,
            MAX_LOOP_LENGTH,
        )
    }

    fn new_with_length(
        sample_rate: f32,
        loopable_region_length: usize,
        max_fade_time: usize,
        max_loop_length: usize,
    ) -> GrainLooper {
        GrainLooper {
            grain_player: GrainPlayer::new_with_length(
                loopable_region_length,
                max_fade_time,
                max_loop_length,
            ),
            loop_scheduler: LoopScheduler::new(),
            is_looping: false,
            sample_rate,

            loop_offset_beats: 0.0,
            fade_duration: 0,

            dry_ramp: RampedValue::new(1.0),
            song_ticks: 0,
            reverse: false,
            speed: 1.0,
            tempo: 120.0,
            beats_per_tick: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.grain_player.reset();
        self.loop_scheduler.reset();
        self.is_looping = false;
        self.song_ticks = 0;
        self.dry_ramp.set(1.0);
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.update_times();
    }

    fn set_tempo(&mut self, bpm: f32) {
        self.tempo = bpm;
        self.update_times();
    }

    pub fn set_fade_time(&mut self, fade: f32) {
        let fade_samples = seconds_to_samples(fade, self.sample_rate);
        assert!(fade_samples <= MAX_FADE_TIME);

        self.fade_duration = fade_samples;
    }

    fn update_times(&mut self) {
        // update everything dependent on tempo and sample rate
        self.beats_per_tick = self.tempo / 60.0 / self.sample_rate;
        self.loop_scheduler.set_fade_lead_in(samples_to_beats(
            self.fade_duration,
            self.tempo,
            self.sample_rate,
        ));
    }

    // offset the loop in the buffer, i.e. "scrub"
    pub fn set_loop_offset(&mut self, offset_beats: f32) {
        self.loop_offset_beats = offset_beats;
    }

    // how long the loop is
    pub fn set_grid(&mut self, duration_seconds: f32) {
        self.loop_scheduler.set_grid_interval(duration_seconds);
    }

    // note that the loop_start_point_seconds is toward the past, as we want to loop something that has already started
    pub fn start_looping(&mut self) {
        self.loop_scheduler.start_looping();
        self.grain_player.start_looping();
    }

    fn schedule_grain(&mut self, wait: usize, duration: usize, offset_reduction: f32) {
        // wait might go away
        self.grain_player.schedule_grain(Grain::new(
            wait,
            beats_to_samples(
                self.loop_offset_beats - offset_reduction,
                self.tempo,
                self.sample_rate,
            ) as f32,
            duration + self.fade_duration,
            self.fade_duration,
            self.reverse,
            self.speed,
        ));
    }

    pub fn stop_looping(&mut self) {
        self.loop_scheduler.stop_looping();
        self.grain_player.stop_looping();
    }

    pub fn set_reverse(&mut self, reverse: bool) {
        self.reverse = reverse;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn tick(&mut self, input: f32) -> f32 {
        // for now we work out the beat time here
        let song_time = samples_to_beats(self.song_ticks, self.tempo, self.sample_rate);
        let events = self.loop_scheduler.tick(song_time);

        for event in events {
            match event {
                LoopEvent::StartGrain { duration } => {
                    print!("EVENT: start grain\n");
                    self.schedule_grain(
                        0,
                        beats_to_samples(duration, self.tempo, self.sample_rate),
                        0.0,
                    );
                    self.is_looping = true;
                }
                LoopEvent::StartLegatoGrain {
                    duration,
                    offset_reduction,
                } => {
                    print!("EVENT: start legato grain\n");
                    self.schedule_grain(
                        0,
                        beats_to_samples(duration, self.tempo, self.sample_rate),
                        offset_reduction,
                    );
                    self.is_looping = true;
                }
                LoopEvent::StopGrain => {
                    // we stop them all
                    print!("EVENT: stop grain\n");
                    self.grain_player.stop_all_grains();
                }
                LoopEvent::FadeInDry => {
                    print!("EVENT: fade in dry\n");
                    self.dry_ramp.ramp(1.0, self.fade_duration);
                }
                LoopEvent::FadeOutDry => {
                    print!("EVENT: fade out dry\n");
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
        looped + dry_level as f32 * dry
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
    use crate::test_utils::all_near;

    struct IncreasingInteger {
        count: usize,
    }

    impl IncreasingInteger {
        pub fn new(start_value: usize) -> Self {
            Self { count: start_value }
        }
    }

    impl Iterator for IncreasingInteger {
        type Item = usize;
        fn next(&mut self) -> Option<usize> {
            let out = self.count;
            self.count += 1;
            Some(out)
        }
    }

    struct GrainLooperFixture {
        pub looper: GrainLooper,
        pub input: IncreasingInteger,
    }

    impl GrainLooperFixture {
        fn new() -> GrainLooperFixture {
            let mut f = GrainLooperFixture {
                looper: GrainLooper::new_with_length(10.0, 20, 4, 10),
                input: IncreasingInteger::new(10),
            };

            f.looper.set_tempo(60.0);
            f
        }

        fn check_output(&mut self, expected: &Vec<f32>) {
            let mut out = vec![];
            for _i in 0..expected.len() {
                out.push(self.looper.tick(self.input.next().unwrap() as f32));
            }
            assert_eq!(out, expected.clone());
        }
    }

    #[test]
    fn test_grain_looper_nicely() {
        let mut looper_fixture = GrainLooperFixture::new();

        let expected1 = (10..15).map(|x| x as f32).collect();

        // checks that the tick returns each of expected
        looper_fixture.check_output(&expected1);

        // can set things on the looper inside the fixture
        looper_fixture.looper.set_grid(0.5);

        // and check again
        let expected2 = (15..20).map(|x| x as f32).collect();
        looper_fixture.check_output(&expected2);
    }

    #[test]
    fn test_grain_looper_loop() {
        // test a 5 sample loop, no fading, not using static buffer yet
        let mut looper_fixture = GrainLooperFixture::new();

        let expected1 = (10..15).map(|x| x as f32).collect();
        looper_fixture.check_output(&expected1);

        looper_fixture.looper.set_fade_time(0.0);
        looper_fixture.looper.set_loop_offset(0.5);
        looper_fixture.looper.set_grid(0.5);
        looper_fixture.looper.start_looping();

        looper_fixture.check_output(&expected1);
        looper_fixture.check_output(&expected1);
        looper_fixture.check_output(&expected1);

        // stop looping
        looper_fixture.looper.stop_looping();

        let expected_back_to_dry = (30..35).map(|x| x as f32).collect();
        looper_fixture.check_output(&expected_back_to_dry);
    }

    #[test]
    fn test_grain_looper_loop_offset() {
        // check that we can change the offset of the loop as its looping
        let mut looper_fixture = GrainLooperFixture::new();

        let expected1 = (10..20).map(|x| x as f32).collect();
        looper_fixture.check_output(&expected1);

        looper_fixture.looper.set_fade_time(0.0);

        // with a tempo of 60 and a sample rate of 10, one sample is 0.1 beats
        looper_fixture.looper.set_loop_offset(0.5);
        looper_fixture.looper.set_grid(0.5);

        looper_fixture.looper.start_looping();

        let expected2 = (15..20).map(|x| x as f32).collect();
        looper_fixture.check_output(&expected2);

        // offset by one earlier compared to the previous loop
        looper_fixture.looper.set_loop_offset(0.6);

        let expected3 = (14..19).map(|x| x as f32).collect();
        looper_fixture.check_output(&expected3);
    }

    #[test]
    fn test_grain_looper_fade_is_flat() {
        // when we loop a DC signal we expect the fades to maintain the DC level
        let mut looper = GrainLooper::new_with_length(10.0, 50, 4, 10);
        looper.set_tempo(60.0);
        let mut out = vec![];
        for _i in 0..8 {
            out.push(looper.tick(1.0));
        }
        // start looping immediately
        // two samples fade
        looper.set_fade_time(0.2);
        // set offset to be the loop length to loop the most recent 5 samples
        looper.set_loop_offset(0.5);
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

    fn overlap_fade(loop_grain_contents: Vec<f32>, fades: Vec<f32>, fade_len: usize) -> Vec<f32> {
        let faded_grain = loop_grain_contents
            .iter()
            .zip(fades.iter())
            .map(|(a, b)| a * b)
            .collect::<Vec<f32>>();

        let mut loop_overlapped = vec![];
        let overlap_len = fades.len() - fade_len;
        for i in 0..overlap_len {
            let overlapped = faded_grain.get(i + overlap_len).unwrap_or(&0.0);
            loop_overlapped.push(faded_grain[i] + overlapped);
        }
        loop_overlapped
    }

    #[test]
    fn test_grain_looper_fade() {
        // test that a single loop fades into the next loop
        let mut looper_fixture = GrainLooperFixture::new();

        let expected1 = (10..20).map(|x| x as f32).collect();
        looper_fixture.check_output(&expected1);

        // two samples fade
        looper_fixture.looper.set_fade_time(0.1);

        // set offset to be the loop length to loop the most recent 4 samples, 16,17,18,19
        looper_fixture.looper.set_loop_offset(0.5);
        looper_fixture.looper.set_grid(0.5);

        looper_fixture.looper.start_looping();

        // loop len 5 but with one sample fade at start and end
        let loop_grain_contents = vec![15.0, 16.0, 17.0, 18.0, 19.0, 20.0];
        let fades = vec![0.5, 1.0, 1.0, 1.0, 1.0, 0.5];

        let loop_overlapped = overlap_fade(loop_grain_contents.clone(), fades.clone(), 1);

        looper_fixture.check_output(&loop_overlapped);
        looper_fixture.check_output(&loop_overlapped);
        looper_fixture.check_output(&loop_overlapped);

        looper_fixture.looper.stop_looping();

        let expected_end = vec![27.5, 36.0, 37.0, 38.0, 39.0, 40.0];

        looper_fixture.check_output(&expected_end);
    }

    #[test]
    fn test_grain_looper_tweak_loop() {
        // test that we can change the offset and length and reversal of the loop
        let mut looper_fixture = GrainLooperFixture::new();

        let expected1 = (10..18).map(|x| x as f32).collect();
        looper_fixture.check_output(&expected1);

        looper_fixture.looper.set_fade_time(0.0);
        // set offset to be the loop length to loop the most recent 4 samples (8,9,10,11)
        looper_fixture.looper.set_loop_offset(0.0);
        looper_fixture.looper.set_grid(0.4);
        looper_fixture.looper.start_looping();

        let first_loop = vec![18.0, 19.0, 20.0, 21.0];
        looper_fixture.check_output(&first_loop);

        // offset the loop backwards by 2 samples (16,17,18)
        looper_fixture.looper.set_loop_offset(0.2);
        looper_fixture.looper.set_grid(0.3);
        // change the length of the loop to be 3 samples (18,17,16)
        looper_fixture.looper.set_reverse(true);
        // slow the loop down by half (18,17.5,17)
        looper_fixture.looper.set_speed(0.5);

        let second_loop = vec![18.0, 17.5, 17.0];

        looper_fixture.check_output(&second_loop);
        looper_fixture.check_output(&second_loop);
        looper_fixture.check_output(&second_loop);
    }

    #[test]
    fn test_grain_looper_immediate_reverse_without_fade() {
        // test that an immediate reverse with a fade does not try to read into the future
        let mut looper_fixture = GrainLooperFixture::new();

        let expected1 = (10..18).map(|x| x as f32).collect();
        looper_fixture.check_output(&expected1);

        looper_fixture.looper.set_fade_time(0.0);
        // set offset to be the loop length to loop the most recent 4 samples (4,5,6,7)
        looper_fixture.looper.set_loop_offset(0.4);
        looper_fixture.looper.set_grid(0.4);
        looper_fixture.looper.set_reverse(true);
        looper_fixture.looper.start_looping();

        let loop_samples = vec![17.0, 16.0, 15.0, 14.0];

        looper_fixture.check_output(&loop_samples);
        looper_fixture.check_output(&loop_samples);
        looper_fixture.check_output(&loop_samples);
    }

    #[test]
    fn test_grain_looper_short_to_long() {
        // test that if a short loop is changed to a longer loop, it still starts in the same place
        let mut looper_fixture = GrainLooperFixture::new();

        let first_len = 0.2;
        let second_len = 0.8;

        looper_fixture.looper.set_fade_time(0.0);
        let initial = vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0];
        looper_fixture.check_output(&initial);

        // set offset to be the loop length to loop the most recent 5 samples
        looper_fixture.looper.set_loop_offset(first_len);
        looper_fixture.looper.set_grid(first_len);
        looper_fixture.looper.start_looping();

        let loop_one = vec![14.0, 15.0];
        looper_fixture.check_output(&loop_one);
        looper_fixture.check_output(&loop_one);

        looper_fixture.looper.set_grid(second_len);

        let end_loop_two = vec![16.0, 17.0, 18.0, 19.0, 20.0, 21.0];
        looper_fixture.check_output(&end_loop_two);

        let loop_two = vec![14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 21.0];

        looper_fixture.check_output(&loop_two);
        looper_fixture.check_output(&loop_two);
        looper_fixture.check_output(&loop_two);
    }
}
