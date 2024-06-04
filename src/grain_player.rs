use crate::grain::Grain;
use crate::{delay_line::DelayLine, stereo_pair::AudioSampleOps};

pub const MAX_GRAINS: usize = 10;

// how much of the buffer we allow to scrub through
// TODO set these to be seconds
const LOOPABLE_REGION_LENGTH: usize = 100000;
const MAX_FADE_TIME: usize = 1000;

pub struct GrainPlayer<T: AudioSampleOps> {
    grains: Vec<Grain>,
    // this is the buffer that is always being written to
    rolling_buffer: DelayLine<T>,
    // this is the buffer that is only written to when looping, and when
    //the loopable region goes out of scope of the rolling buffer we switch to this one
    static_buffer: DelayLine<T>,

    // ticks up as the rolling buffer scrolls left
    rolling_offset: usize,
    use_static_buffer: bool,
    loopable_region_length: usize,
    fade_allowance: usize,
    is_filling_static_buffer: bool,
}

// schedule and play grains
// handles the rolling and static buffers so that existing loopable region is frozen when looping for along time,
//  whilst at the same time new content is instantly available
#[allow(dead_code)]
impl<T: AudioSampleOps> GrainPlayer<T> {
    pub fn new(sample_rate: f32) -> GrainPlayer<T> {
        GrainPlayer::new_with_length(sample_rate, LOOPABLE_REGION_LENGTH, MAX_FADE_TIME)
    }

    pub fn new_with_length(
        sample_rate: f32,
        loopable_region_length: usize,
        max_fade_time: usize,
    ) -> GrainPlayer<T> {
        let delay_line_length_rolling = loopable_region_length * 2 + max_fade_time;
        let delay_line_length_static = loopable_region_length + max_fade_time;
        let delay_line_rolling = DelayLine::new(delay_line_length_rolling);
        let delay_line_static = DelayLine::new(delay_line_length_static);

        let mut grains_init = vec![];
        for _ in 0..MAX_GRAINS {
            grains_init.push(Grain::new(0, 0.0, 0, 0, false, 0.0));
        }

        GrainPlayer {
            grains: grains_init,
            rolling_buffer: delay_line_rolling,
            static_buffer: delay_line_static,
            rolling_offset: 0,
            use_static_buffer: false,
            loopable_region_length: loopable_region_length,
            fade_allowance: max_fade_time,
            is_filling_static_buffer: false,
        }
    }

    pub fn schedule_grain(&mut self, grain: Grain) {
        // replace a finished grain
        for i in 0..self.grains.len() {
            if self.grains[i].is_finished() {
                self.grains[i] = grain;
                return;
            }
        }
    }

    pub fn start_looping(&mut self) {
        self.is_filling_static_buffer = true;
        self.use_static_buffer = false;
        self.rolling_offset = 0;
    }

    pub fn stop_looping(&mut self) {
        self.is_filling_static_buffer = false;
        self.use_static_buffer = false;
        self.rolling_offset = 0;
    }

    pub fn tick(&mut self, input: T) -> T {
        self.rolling_buffer.tick(input);
        self.rolling_offset += 1;
        self.tick_static_buffer_copy(input);

        let out;

        if self.use_static_buffer {
            out = GrainPlayer::<T>::read_grains(
                &mut self.grains,
                &self.static_buffer,
                self.fade_allowance,
            );
        } else {
            out = GrainPlayer::<T>::read_grains(
                &mut self.grains,
                &self.rolling_buffer,
                self.rolling_offset,
            );
        }
        out
    }

    fn read_grains(grains: &mut Vec<Grain>, delay_line: &DelayLine<T>, rolling_offset: usize) -> T {
        let mut out = Default::default();

        // accumulate output of all grains
        for grain in grains.iter_mut() {
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
                rolling_offset,
            );

            out = out + delay_line.read_interpolated(delay) * amplitude;
        }
        out
    }

    // that when the loopable region exits the rolling buffer, we can use the static one
    fn tick_static_buffer_copy(&mut self, input: T) {
        // don't tick it if its full and we're using it, or if we're not looping
        if self.use_static_buffer || !self.is_filling_static_buffer {
            return;
        }
        // fill the static buffer with the loop region
        // we do this by reading the rolling buffer at a delay of the loopable region
        self.static_buffer
            .tick(self.rolling_buffer.read(self.loopable_region_length));

        // when the rolling offset has reached the end of the loopable region, and the fade allowance
        // we switch to the static buffer
        if self.rolling_offset >= self.ticks_before_switch_to_static_buffer() {
            self.is_filling_static_buffer = false;
            self.use_static_buffer = true;
        }
    }

    // todo: alternative tick that can loop over a delay line of
    // things that can't be interpolated or whatnot  might need different impl

    fn ticks_before_switch_to_static_buffer(&self) -> usize {
        self.loopable_region_length + self.fade_allowance
    }

    fn is_filling_static_buffer(&self) -> bool {
        self.is_filling_static_buffer
    }

    fn is_using_static_buffer(&self) -> bool {
        self.use_static_buffer
    }

    fn static_buffer(&self) -> &DelayLine<T> {
        &self.static_buffer
    }

    fn rolling_buffer(&self) -> &DelayLine<T> {
        &self.rolling_buffer
    }

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

    pub fn loopable_region_length(&self) -> usize {
        self.loopable_region_length
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    const sample_rate: f32 = 10.0;

    #[test]
    fn test_grain_player_state() {
        let mut player = GrainPlayer::new_with_length(sample_rate, 100, 10);

        player.schedule_grain(Grain::new(2, 10.0, 4, 0, false, 1.0));

        assert_eq!(player.num_scheduled_grains(), 1);
        assert_eq!(player.num_playing_grains(), 0);
        assert_eq!(player.num_finished_grains(), MAX_GRAINS - 1);

        // tick past wait time
        for _ in 0..2 {
            player.tick(0.0);
        }

        assert_eq!(player.num_scheduled_grains(), 0);
        assert_eq!(player.num_playing_grains(), 1);
        assert_eq!(player.num_finished_grains(), MAX_GRAINS - 1);

        // tick past duration
        for _ in 0..4 {
            player.tick(0.0);
        }
        assert_eq!(player.num_scheduled_grains(), 0);
        assert_eq!(player.num_playing_grains(), 0);
        assert_eq!(player.num_finished_grains(), MAX_GRAINS);
    }

    #[test]
    fn test_grain_player_stop_all() {
        let mut player = GrainPlayer::new_with_length(sample_rate, 100, 10);

        player.schedule_grain(Grain::new(0, 10.0, 4, 2, false, 1.0));
        player.schedule_grain(Grain::new(0, 10.0, 10, 2, false, 1.0));

        assert_eq!(player.num_playing_grains(), 2);

        player.tick(0.0);

        assert_eq!(player.num_playing_grains(), 2);

        player.stop_all_grains();

        player.tick(0.0);

        // grains keep going until fade is finished
        assert_eq!(player.num_playing_grains(), 2);

        player.tick(0.0);
        player.tick(0.0);

        assert_eq!(player.num_playing_grains(), 0);
        assert_eq!(player.num_finished_grains(), 10);
    }

    #[test]
    fn test_grain_player_dry_grain() {
        let mut player = GrainPlayer::<f32>::new_with_length(sample_rate, 10, 0);

        // if we schedule a grain with an offset of 0 it should just ouput the input
        player.schedule_grain(Grain::new(0, 0.0, 20, 0, false, 1.0));

        let N = 10;

        let input: Vec<f32> = (0..N).map(|x| x as f32).collect();
        let mut input_iter = input.iter();

        let mut output = vec![];
        // tick past wait time
        for _ in 0..N {
            output.push(player.tick(*input_iter.next().unwrap()));
        }

        assert_eq!(output, input);
    }

    #[test]
    fn test_grain_player_static_buffer_states() {
        let mut player = GrainPlayer::<f32>::new_with_length(sample_rate, 10, 0);
        let p = 10;
        let pre_input: Vec<f32> = (0..p).map(|x| x as f32).collect();
        for input in pre_input.iter() {
            player.tick(*input);
        }
        player.start_looping();

        let input: Vec<f32> = (0..20).map(|x| (x + 10) as f32).collect();
        let mut input_iter = input.iter();

        let mut output = vec![];
        // for first 10 samples (loopable region length) we should be filling the static buffer but not using it
        for i in 0..10 {
            assert!(
                !player.is_using_static_buffer(),
                "is using static buffer after {}",
                i
            );
            assert!(player.is_filling_static_buffer());
            output.push(player.tick(*input_iter.next().unwrap()));
        }
        // the static buffer should now be filled with the first 10 samples
        assert!(!player.is_filling_static_buffer());
        let expected_static = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let static_buffer = player.static_buffer().buffer().clone();
        assert_eq!(*static_buffer, expected_static);

        // the next ten samples still tick the rolling buffer but not the static one
        for _i in 0..10 {
            assert!(player.is_using_static_buffer());
            assert!(!player.is_filling_static_buffer());
            output.push(player.tick(*input_iter.next().unwrap()));
        }
        // static buffer should still be the same
        assert_eq!(*static_buffer, expected_static);
        // rolling buffer has new stuff in
        let mut expected_rolling1: Vec<f32> = (10..20).map(|x| (x + 10) as f32).collect();
        let expected_rolling2: Vec<f32> = (10..20).map(|x| x as f32).collect();
        expected_rolling1.extend(expected_rolling2);
        let rolling_buffer = player.rolling_buffer().buffer().clone();
        assert_eq!(*rolling_buffer, expected_rolling1);

        // no grains were scheduled so the output should be zero
        assert_eq!(output, vec![0.0; 20]);
    }

    #[test]
    fn test_grain_player_output() {
        let mut player = GrainPlayer::<f32>::new_with_length(sample_rate, 10, 0);
        let N = 20;
        let pre_input: Vec<f32> = (0..N).map(|x| x as f32).collect();
        for input in pre_input.iter() {
            player.tick(*input);
        }

        player.start_looping();
        let input: Vec<f32> = (N..2 * N).map(|x| (x + 10) as f32).collect();

        // once looping all grains with the same offset should output the same thing

        // this grain reads the rolling buffer
        player.schedule_grain(Grain::new(2, 10.0, 3, 0, false, 1.0));
        let expected_g1 = vec![0.0, 0.0, 10.0, 11.0, 12.0];

        // this grain reads both the rolling buffer and then the static buffer
        player.schedule_grain(Grain::new(8, 10.0, 3, 0, false, 1.0));
        let expected_g2 = vec![0.0, 0.0, 0.0, 10.0, 11.0, 12.0];

        // this grain reads the static buffer
        player.schedule_grain(Grain::new(14, 10.0, 3, 0, false, 1.0));
        let expected_g3 = vec![0.0, 0.0, 0.0, 10.0, 11.0, 12.0];

        let mut input_iter = input.iter();

        let mut out1 = vec![];
        for _ in expected_g1.iter() {
            out1.push(player.tick(*input_iter.next().unwrap()));
        }

        assert_eq!(out1, expected_g1);

        let mut out2 = vec![];
        for _ in expected_g2.iter() {
            out2.push(player.tick(*input_iter.next().unwrap()));
        }
        assert_eq!(out2, expected_g2);

        let mut out3 = vec![];
        for _ in expected_g3.iter() {
            out3.push(player.tick(*input_iter.next().unwrap()));
        }
        assert_eq!(out3, expected_g3);
    }
    /*

    #[test]
    fn test_grain_looper_switch_to_static() {
        // the loopable region is only 4 samples long
        let mut looper = GrainPlayer::new_with_length(8.0, 4, 0);
        let mut out = vec![];

        let loop_start = 4;
        let ticks_before_switch_to_static_buffer = looper.ticks_before_switch_to_static_buffer();

        // fill the rolling buffer with 4 samples
        for i in 0..loop_start {
            out.push(looper.tick((i + 10) as f32));
        }
        // start looping those 4 samples
        // looper.set_fade_time(0.0);
        // looper.set_loop_offset(0.5);
        // looper.set_loop_duration(0.5);
        // looper.start_looping(0.5);

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
        let mut looper = GrainPlayer::new_with_length(8.0, 8, 2);
        let mut out = vec![];

        let loop_start = 6;
        let ticks_before_switch_to_static_buffer = looper.ticks_before_switch_to_static_buffer();

        // fill the rolling buffer with 4 samples
        for i in 0..loop_start {
            out.push(looper.tick((i + 10) as f32));
        }
        // set fade to two samples
        // looper.set_fade_time(0.25);
        // looper.set_loop_offset(0.5);
        // looper.set_loop_duration(0.5);
        // looper.start_looping(0.5);

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
    */
}
