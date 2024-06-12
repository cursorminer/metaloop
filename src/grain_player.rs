use crate::grain::Grain;
use crate::{delay_line::DelayLine, stereo_pair::AudioSampleOps};

pub const MAX_GRAINS: usize = 10;

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
    static_buffer_margin: usize,
    is_filling_static_buffer: bool,
}

// schedule and play grains
// handles the rolling and static buffers so that existing loopable region is frozen when looping for along time,
//  whilst at the same time new content is instantly available
#[allow(dead_code)]
impl<T: AudioSampleOps> GrainPlayer<T> {
    pub fn new_with_length(
        loopable_region_length: usize,
        max_fade_time: usize,
        max_loop_time: usize,
    ) -> GrainPlayer<T> {
        //static buffer must have at least the loopable region, with fade and max loop time
        let delay_line_length_static = loopable_region_length + max_fade_time + max_loop_time;
        // rolling buffer must be length of loopable region plus the static buffer
        let delay_line_length_rolling = loopable_region_length + delay_line_length_static;
        let delay_line_static = DelayLine::new(delay_line_length_static);
        let delay_line_rolling = DelayLine::new(delay_line_length_rolling);

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
            static_buffer_margin: max_fade_time + max_loop_time,
            is_filling_static_buffer: false,
        }
    }

    pub fn schedule_grain(&mut self, grain: Grain) {
        // todo look at all the params and make sure it will not read beyond the buffer
        for i in 0..self.grains.len() {
            if self.grains[i].is_finished() {
                self.grains[i] = grain;
                return;
            }
        }
    }

    pub fn reset(&mut self) {
        self.rolling_buffer.reset();
        self.static_buffer.reset();
        self.is_filling_static_buffer = false;
        self.use_static_buffer = false;
        self.rolling_offset = 0;
    }

    // the offset of the grain doesn't mean anything unless we have a
    // reference point to when we started looping.
    // this is the rolling offset
    // it kind of sucks
    pub fn start_looping(&mut self) {
        self.is_filling_static_buffer = true;
        self.use_static_buffer = false;
        self.rolling_offset = 0;
    }

    pub fn stop_looping(&mut self) {
        self.is_filling_static_buffer = false;
        self.use_static_buffer = false;
    }

    pub fn tick(&mut self, input: T) -> T {
        self.rolling_buffer.tick(input);
        self.rolling_offset += 1;
        self.tick_static_buffer_copy();

        let out;

        if self.use_static_buffer {
            out = GrainPlayer::<T>::read_grains(
                &mut self.grains,
                &self.static_buffer,
                self.static_buffer_margin,
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

            if delay >= 0.0 && delay < delay_line.len() as f32 {
                out = out + delay_line.read_interpolated(delay) * amplitude;
            } else {
                debug_assert!(
                    delay >= 0.0 && delay < delay_line.len() as f32,
                    "delay is outside buffer. delay_pos: {:?}, rolling_offset: {:?}",
                    delay_pos,
                    rolling_offset,
                );
            }
        }
        out
    }

    // that when the loopable region exits the rolling buffer, we can use the static one
    fn tick_static_buffer_copy(&mut self) {
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
        self.static_buffer.len()
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
    use crate::test_utils::all_near;

    #[test]
    fn test_grain_player_state() {
        let mut player = GrainPlayer::new_with_length(100, 10, 10);

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
        let mut player = GrainPlayer::new_with_length(100, 10, 10);

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
        let mut player = GrainPlayer::<f32>::new_with_length(10, 0, 10);

        // if we schedule a grain with an offset of 0 it should just ouput the input
        player.schedule_grain(Grain::new(0, 0.0, 20, 0, false, 1.0));

        let num_samples = 10;

        let input: Vec<f32> = (0..num_samples).map(|x| x as f32).collect();
        let mut input_iter = input.iter();

        let mut output = vec![];
        // tick past wait time
        for _ in 0..num_samples {
            output.push(player.tick(*input_iter.next().unwrap()));
        }

        assert_eq!(output, input);
    }

    #[test]
    fn test_grain_player_static_buffer_states() {
        let mut player = GrainPlayer::<f32>::new_with_length(8, 0, 2);
        let p = 10;
        let pre_input: Vec<f32> = (0..p).map(|x| x as f32).collect();
        for input in pre_input.iter() {
            player.tick(*input);
        }
        player.start_looping();

        let input: Vec<f32> = (0..20).map(|x| (x + 10) as f32).collect();
        let mut input_iter = input.iter();

        let mut output = vec![];
        // for first 10 samples (loopable region length + max loop) we should be filling the static buffer but not using it
        for i in 0..10 {
            assert!(
                !player.is_using_static_buffer(),
                "is using static buffer after {}",
                i
            );
            assert!(player.is_filling_static_buffer());
            output.push(player.tick(*input_iter.next().unwrap()));
        }

        // the static buffer should now be filled with the most recent loopable region
        assert!(!player.is_filling_static_buffer());
        let expected_static = vec![2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0];
        let static_buffer = player.static_buffer().buffer().clone();
        assert_eq!(*static_buffer, expected_static);

        for _i in 0..10 {
            assert!(player.is_using_static_buffer());
            assert!(!player.is_filling_static_buffer());
            output.push(player.tick(*input_iter.next().unwrap()));
        }
        // static buffer should still be the same
        assert_eq!(*static_buffer, expected_static);
        // rolling buffer has new stuff in
        let mut expected_rolling1: Vec<f32> = (18..30).map(|x| x as f32).collect();
        let expected_rolling2: Vec<f32> = (12..18).map(|x| x as f32).collect();
        expected_rolling1.extend(expected_rolling2);
        let rolling_buffer = player.rolling_buffer().buffer().clone();
        assert_eq!(*rolling_buffer, expected_rolling1);

        // no grains were scheduled so the output should be zero
        assert_eq!(output, vec![0.0; 20]);
    }

    #[test]
    fn test_grain_player_output() {
        let mut player = GrainPlayer::<f32>::new_with_length(10, 0, 10);

        // fill buffer with initial 10 samples
        let n_pre_input = 10;
        let pre_input: Vec<f32> = (0..n_pre_input).map(|x| x as f32).collect();
        for input in pre_input.iter() {
            player.tick(*input);
        }

        player.start_looping();
        let n_input = n_pre_input + 5 + 6 + 6;
        let input: Vec<f32> = (n_pre_input..n_input).map(|x| x as f32).collect();

        // once looping all grains with the same offset should output the same thing

        // this grain reads the rolling buffer
        player.schedule_grain(Grain::new(2, 5.0, 3, 0, false, 1.0));
        let expected_g1 = vec![0.0, 0.0, 5.0, 6.0, 7.0];

        // this grain reads both the rolling buffer and then the static buffer
        player.schedule_grain(Grain::new(8, 5.0, 3, 0, false, 1.0));
        let expected_g2 = vec![0.0, 0.0, 0.0, 5.0, 6.0, 7.0];

        // this grain reads the static buffer
        player.schedule_grain(Grain::new(14, 5.0, 3, 0, false, 1.0));
        let expected_g3 = vec![0.0, 0.0, 0.0, 5.0, 6.0, 7.0];

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

    #[test]
    fn test_grain_player_output_fade() {
        // set a max fade time of 2
        // check that it can be used
        let mut player = GrainPlayer::<f32>::new_with_length(10, 4, 10);
        let n_pre_input = 10;
        let pre_input: Vec<f32> = (0..n_pre_input).map(|x| x as f32).collect();
        for input in pre_input.iter() {
            player.tick(*input);
        }

        let n_input = n_pre_input + 6 + 6 + 6;
        player.start_looping();
        let input: Vec<f32> = (n_pre_input..n_input).map(|x| (x + 10) as f32).collect();

        // once looping all grains with the same offset should output the same thing
        let fade = 1;

        // this grain reads the rolling buffer
        player.schedule_grain(Grain::new(2, 5.0, 4, fade, false, 1.0));

        // wrong...?
        let expected_g1 = vec![0.0, 0.0, 2.5, 6.0, 7.0, 4.0];

        // this grain reads both the rolling buffer and then the static buffer
        player.schedule_grain(Grain::new(8, 5.0, 4, fade, false, 1.0));
        let expected_g2 = vec![0.0, 0.0, 2.5, 6.0, 7.0, 4.0];

        // this grain reads the static buffer
        player.schedule_grain(Grain::new(14, 5.0, 4, fade, true, 1.0));
        let expected_g3 = vec![0.0, 0.0, 4.0, 7.0, 6.0, 2.5];

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

    #[test]
    fn test_grain_player_immediate_reverse_with_fade() {
        // test that an immediate reverse with a fade does not try to read into the future
        let mut player = GrainPlayer::new_with_length(50, 4, 10);
        let mut out = vec![];

        let loop_start_at = 8;
        let stop_at = 12;

        for i in 0..loop_start_at {
            out.push(player.tick(i as f32));
        }

        player.start_looping();
        // set offset to be the loop length to loop the most recent 4 samples (4,5,6,7)
        player.schedule_grain(Grain::new(0, 4.0, 4, 1, true, 1.0));

        for i in loop_start_at..stop_at {
            out.push(player.tick(i as f32));
        }

        let mut expected = vec![0.0; 8];

        let grain_samples = vec![3.5, 6.0, 5.0, 2.0];

        expected.extend(&grain_samples);

        assert_eq!(out, expected);
        all_near(&out, &expected, 0.0001);
    }

    fn test_grain_player_lengthen_grain() {
        // test the scenario where the grain is lengthened when already using the static buffer
    }
}
