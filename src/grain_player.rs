use crate::grain::Grain;
use crate::grain::WhichBuffer;
use crate::{delay_line::DelayLine, stereo_pair::AudioSampleOps};

pub const MAX_GRAINS: usize = 10;

pub struct GrainPlayer<T: AudioSampleOps> {
    grains: Vec<Grain>,

    buffer_a: DelayLine<T>,
    buffer_b: DelayLine<T>,

    // ticks up as the rolling buffer scrolls left
    rolling_offset_a: usize,
    rolling_offset_b: usize,

    // the length of the part of the buffer we can loop over
    loopable_region_length: usize,

    // If we have been looping a long time, we should freeze the buffer so that we can still read the old loopable region
    frozen_buffer: WhichBuffer,
    start_grains_buffer: WhichBuffer,

    is_looping: bool,
}

// schedule and play grains
// handles the rolling and static buffers so that existing loopable region is frozen when looping for a long time,
//  whilst at the same time new content is instantly available
#[allow(dead_code)]
impl<T: AudioSampleOps> GrainPlayer<T> {
    pub fn new_with_length(
        loopable_region_length: usize,
        max_fade_time: usize,
        max_loop_time: usize,
    ) -> GrainPlayer<T> {
        assert!(
            loopable_region_length >= max_fade_time + max_loop_time,
            "buffer length logic assumes that loop plus fade is smaller than tail of loop"
        );

        let delay_line_length = loopable_region_length * 2 + max_fade_time + max_loop_time;

        let delay_line_a = DelayLine::new(delay_line_length);
        let delay_line_b = DelayLine::new(delay_line_length);

        let mut grains_init = vec![];
        for _ in 0..MAX_GRAINS {
            grains_init.push(Grain::new(0.0, 0, 0, false, 0.0));
        }

        GrainPlayer {
            grains: grains_init,
            buffer_a: delay_line_a,
            buffer_b: delay_line_b,
            rolling_offset_a: 0,
            rolling_offset_b: 0,
            loopable_region_length: loopable_region_length,
            frozen_buffer: WhichBuffer::Neither,
            start_grains_buffer: WhichBuffer::A,
            is_looping: false,
        }
    }

    pub fn schedule_grain(&mut self, grain: Grain) {
        // todo look at all the params and make sure it will not read beyond the buffer
        for i in 0..self.grains.len() {
            if self.grains[i].is_finished() {
                self.grains[i] = grain;
                self.grains[i].set_which_buffer(self.start_grains_buffer);
                return;
            }
        }
    }

    pub fn reset(&mut self) {
        self.buffer_a.reset();
        self.buffer_b.reset();

        self.frozen_buffer = WhichBuffer::Neither;

        self.rolling_offset_a = 0;
        self.rolling_offset_b = 0;
    }

    // the offset of the grain doesn't mean anything unless we have a
    // reference point to when we started looping, as the rolling buffer is constantly moving along
    // this is the rolling offset
    // it kind of sucks
    pub fn start_looping(&mut self) {
        // reset the rolling offsets on the new grain buffer
        match self.start_grains_buffer {
            WhichBuffer::A => {
                self.rolling_offset_a = 0;
            }
            WhichBuffer::B => {
                self.rolling_offset_b = 0;
            }
            WhichBuffer::Neither => {
                assert!(false);
            }
        }

        self.is_looping = true;
    }

    pub fn stop_looping(&mut self) {
        // indicate we should start new grains on the other buffer that was not frozen
        match self.start_grains_buffer {
            WhichBuffer::A => {
                self.start_grains_buffer = WhichBuffer::B;
            }
            WhichBuffer::B => {
                self.start_grains_buffer = WhichBuffer::A;
            }
            WhichBuffer::Neither => {
                assert!(false);
            }
        }

        // we can unfreeze the currently frozen buffer, as we know there will be no more grains triggered on it
        self.frozen_buffer = WhichBuffer::Neither;
        self.is_looping = false;
    }

    pub fn tick(&mut self, input: T) -> T {
        if self.frozen_buffer != WhichBuffer::A {
            self.buffer_a.tick(input);
            self.rolling_offset_a += 1;
        }
        if self.frozen_buffer != WhichBuffer::B {
            self.buffer_b.tick(input);
            self.rolling_offset_b += 1;
        }

        self.freeze_buffer_if_needed();

        let mut out = Default::default();

        for grain in self.grains.iter_mut() {
            if grain.is_finished() {
                continue;
            }
            if grain.which_buffer() == WhichBuffer::A {
                out = out
                    + GrainPlayer::<T>::read_grain(grain, &self.buffer_a, self.rolling_offset_a);
            } else {
                out = out
                    + GrainPlayer::<T>::read_grain(grain, &self.buffer_b, self.rolling_offset_b);
            }
        }

        out
    }

    fn read_grain(grain: &mut Grain, delay_line: &DelayLine<T>, rolling_offset: usize) -> T {
        let mut out = Default::default();

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

        out
    }

    fn freeze_buffer_if_needed(&mut self) {
        // if the rolling offset has hit the length of the buffer, freeze it
        // this only applies if we are looping and for the current new grains buffer
        if self.is_looping {
            match self.start_grains_buffer {
                WhichBuffer::A => {
                    if self.rolling_offset_a >= self.loopable_region_length {
                        self.frozen_buffer = WhichBuffer::A;
                    }
                }
                WhichBuffer::B => {
                    if self.rolling_offset_b >= self.loopable_region_length {
                        self.frozen_buffer = WhichBuffer::B;
                    }
                }
                WhichBuffer::Neither => {
                    assert!(false);
                }
            }
        }
    }

    pub fn stop_all_grains(&mut self) {
        for grain in self.grains.iter_mut() {
            grain.stop();
        }
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

    pub fn frozen_buffer(&self) -> WhichBuffer {
        self.frozen_buffer
    }

    pub fn start_grains_buffer(&self) -> WhichBuffer {
        self.start_grains_buffer
    }

    pub fn buffer_a(&self) -> &DelayLine<T> {
        &self.buffer_a
    }

    pub fn buffer_b(&self) -> &DelayLine<T> {
        &self.buffer_b
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    const SAMPLE_RATE: f32 = 10.0;
    use crate::test_utils::all_near;

    #[test]
    fn test_grain_player_state() {
        let mut player = GrainPlayer::new_with_length(100, 10, 10);

        player.schedule_grain(Grain::new(10.0, 4, 0, false, 1.0));

        assert_eq!(player.num_playing_grains(), 1);
        assert_eq!(player.num_finished_grains(), MAX_GRAINS - 1);

        // tick past duration
        for _ in 0..4 {
            player.tick(0.0);
        }
        assert_eq!(player.num_playing_grains(), 0);
        assert_eq!(player.num_finished_grains(), MAX_GRAINS);
    }

    #[test]
    fn test_grain_player_stop_all() {
        let mut player = GrainPlayer::new_with_length(100, 10, 10);

        player.schedule_grain(Grain::new(10.0, 4, 2, false, 1.0));
        player.schedule_grain(Grain::new(10.0, 10, 2, false, 1.0));

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
        player.schedule_grain(Grain::new(0.0, 20, 0, false, 1.0));

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
    fn test_grain_player_buffer_states() {
        let mut player = GrainPlayer::<f32>::new_with_length(8, 0, 2);

        // put 10 initial samples in
        let p = 10;
        let pre_input: Vec<f32> = (0..p).map(|x| x as f32).collect();
        for input in pre_input.iter() {
            player.tick(*input);
        }

        // check pre loop state
        assert!(player.start_grains_buffer() == WhichBuffer::A);
        assert!(player.frozen_buffer() == WhichBuffer::Neither);

        player.start_looping();

        let input: Vec<f32> = (0..20).map(|x| (x + 10) as f32).collect();
        let mut input_iter = input.iter();

        let mut output = vec![];

        // for first 8 samples after starting looping (loopable region length) we should be filling both buffers, freezing neither, starting grains on buffer A
        for i in 0..8 {
            // expect grain buffer to be A,
            assert!(
                player.frozen_buffer() == WhichBuffer::Neither,
                "has frozen something after {}",
                i
            );
            assert!(player.start_grains_buffer() == WhichBuffer::A);
            assert!(player.frozen_buffer() == WhichBuffer::Neither);
            output.push(player.tick(*input_iter.next().unwrap()));
        }

        // both buffer should now be filled with the loopable region
        assert!(player.frozen_buffer == WhichBuffer::A);
        let expected_frozen = vec![
            0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
            16.0, 17.0,
        ];
        let frozen_buffer = player.buffer_a().buffer().clone();
        assert_eq!(*frozen_buffer, expected_frozen);

        for _i in 0..10 {
            assert!(player.frozen_buffer == WhichBuffer::A);
            assert!(player.start_grains_buffer == WhichBuffer::A);
            output.push(player.tick(*input_iter.next().unwrap()));
        }
        // buffer A still has loopable region
        let frozen_buffer = player.buffer_a().buffer().clone();
        assert_eq!(*frozen_buffer, expected_frozen);

        // buffer B kept on rollin' we expect the latest values in there
        let expected_rolling = vec![
            18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0, 10.0, 11.0, 12.0, 13.0,
            14.0, 15.0, 16.0, 17.0,
        ];
        let rolling_buffer = player.buffer_b().buffer().clone();
        assert_eq!(*rolling_buffer, expected_rolling);

        // no grains were scheduled so the output should be zero
        assert_eq!(output, vec![0.0; 18]);
    }

    #[test]
    fn test_grain_player_output_nofade() {
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

        let mut input_iter = input.iter();

        player.tick(*input_iter.next().unwrap());
        player.tick(*input_iter.next().unwrap());
        // once looping all grains with the same offset should output the same thing

        // this grain reads the rolling buffer
        player.schedule_grain(Grain::new(5.0, 3, 0, false, 1.0));
        let expected_g1 = vec![5.0, 6.0, 7.0, 0.0, 0.0, 0.0];

        let mut out1 = vec![];
        for _ in expected_g1.iter() {
            out1.push(player.tick(*input_iter.next().unwrap()));
        }
        assert_eq!(out1, expected_g1);

        // this grain reads both the rolling buffer and then the static buffer
        player.schedule_grain(Grain::new(5.0, 3, 0, false, 1.0));
        let expected_g2 = vec![5.0, 6.0, 7.0, 0.0, 0.0, 0.0];

        let mut out2 = vec![];
        for _ in expected_g2.iter() {
            out2.push(player.tick(*input_iter.next().unwrap()));
        }
        assert_eq!(out2, expected_g2);

        // this grain reads the static buffer, despite the fact we switched back to the rolling buffer half way through
        player.schedule_grain(Grain::new(5.0, 3, 0, false, 1.0));
        let expected_g3 = vec![5.0, 6.0, 7.0];

        let mut out3 = vec![];
        for _ in expected_g3.iter() {
            let input = *input_iter.next().unwrap();
            out3.push(player.tick(input));
            if input == 25.0 {
                player.stop_looping();
            }
        }
        assert_eq!(out3, expected_g3);
    }

    #[test]
    fn test_grain_player_output_fade() {
        // set a max fade time of 2
        // check that it can be used
        let mut player = GrainPlayer::<f32>::new_with_length(10, 4, 5);
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

        let mut input_iter = input.iter();
        // tick twice
        player.tick(*input_iter.next().unwrap());
        player.tick(*input_iter.next().unwrap());

        // this grain reads the rolling buffer
        player.schedule_grain(Grain::new(5.0, 4, fade, false, 1.0));

        // wrong...?
        let expected_g1 = vec![2.5, 6.0, 7.0, 4.0, 0.0, 0.0];
        let mut out1 = vec![];
        for _ in expected_g1.iter() {
            out1.push(player.tick(*input_iter.next().unwrap()));
        }
        assert_eq!(out1, expected_g1);

        // this grain reads both the rolling buffer and then the static buffer
        player.schedule_grain(Grain::new(5.0, 4, fade, false, 1.0));
        let expected_g2 = vec![2.5, 6.0, 7.0, 4.0, 0.0, 0.0];
        let mut out2 = vec![];
        for _ in expected_g2.iter() {
            out2.push(player.tick(*input_iter.next().unwrap()));
        }
        assert_eq!(out2, expected_g2);

        // this grain reads the static buffer, despite the fact we switched back to the rolling buffer half way through
        player.schedule_grain(Grain::new(5.0, 4, fade, true, 1.0));
        let expected_g3 = vec![4.0, 7.0, 6.0, 2.5];

        let mut out3 = vec![];
        for _ in expected_g3.iter() {
            let input = *input_iter.next().unwrap();
            out3.push(player.tick(input));
            if input == 17.0 {
                player.stop_looping();
            }
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
        player.schedule_grain(Grain::new(4.0, 4, 1, true, 1.0));

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

    #[test]
    fn test_grain_start_stop_too_fast() {
        // this tests the situation where we have a frozen buffer, but we try to start two new looping sessions
        // within a single fade. This is tricky because the fade out assumes a frozen buffer, and two stop starts
        //will unfreeze both buffers.
        let mut player = GrainPlayer::<f32>::new_with_length(20, 5, 5);
        let n_pre_input = 10;
        let pre_input: Vec<f32> = (0..n_pre_input).map(|x| x as f32).collect();
        for input in pre_input.iter() {
            player.tick(*input);
        }

        let n_input = n_pre_input + 20 + 6 + 6 + 6;
        player.start_looping();
        let input: Vec<f32> = (n_pre_input..n_input).map(|x| (x + 10) as f32).collect();
        let mut input_iter = input.iter();

        // tick until buffer A is frozen
        for _ in 0..20 {
            player.tick(*input_iter.next().unwrap());
        }

        // once looping all grains with the same offset should output the same thing
        let fade = 5;

        // this grain reads the rolling buffer
        player.schedule_grain(Grain::new(6.0, 6, fade, false, 1.0));

        // a build up of multiple fading grains
        let expected = vec![1.0, 2.5, 15.0, 56.25, 65.0, 62.25, 41.75, 24.75, 6.75];
        let mut out1 = vec![];

        out1.push(player.tick(*input_iter.next().unwrap()));

        // stop and start will unfreeze A, which has a playing clip on it, but rolling_offset_a will still be valid
        player.stop_looping();
        out1.push(player.tick(*input_iter.next().unwrap()));
        player.start_looping();
        player.schedule_grain(Grain::new(0.0, 6, fade, false, 1.0));

        // stop and start will unfreeze B, and switch to making new grain on A, which will reset rolling_offset_a
        player.stop_looping();
        out1.push(player.tick(*input_iter.next().unwrap()));
        player.start_looping();
        player.schedule_grain(Grain::new(20.0, 6, fade, false, 1.0));

        for _ in 0..6 {
            out1.push(player.tick(*input_iter.next().unwrap()));
        }

        assert_eq!(out1, expected);
    }
}
