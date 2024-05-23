use crate::delay_line::{self, DelayLine};
use crate::grain_player::GrainPlayer;

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
}

impl GrainLooper {
    fn new(sample_rate: f32) -> GrainLooper {
        let delay_lineA = DelayLine::new(DELAY_LINE_LENGTH);
        let delay_lineB = DelayLine::new(DELAY_LINE_LENGTH);
        GrainLooper {
            grain_player: GrainPlayer::new(sample_rate),
            is_looping: false,
            sample_rate,
            ticks_till_next_loop: std::usize::MAX,
            rolling_buffer: delay_lineA,
            static_buffer: delay_lineB,
            rolling_offset: 0,
            use_static_buffer: false,
        }
    }

    pub fn start_looping(&mut self, loop_start_time: f32, loop_duration: f32) {
        self.is_looping = true;
        // swap the buffers

        // schedule the first grain
        let wait = (loop_start_time * self.sample_rate) as usize;

        // for now the offset is always the same as the duration
        let duration = (loop_duration * self.sample_rate) as usize;
        let offset = duration;
        self.grain_player.schedule_grain(wait, offset, duration);

        self.ticks_till_next_loop = wait + duration;
        self.rolling_offset = 0;
        self.use_static_buffer = false;
    }

    pub fn stop_looping(&mut self, loop_stop_time: f32) {
        self.is_looping = false;
        self.ticks_till_next_loop = std::usize::MAX;
    }

    fn tick(&mut self, input: f32) -> f32 {
        self.rolling_buffer.tick(input);
        let mut out = 0.0;

        if self.is_looping {
            self.ticks_till_next_loop -= 1;
            self.tick_static_buffer();

            if self.use_static_buffer {
                out = self.grain_player.tick(&self.static_buffer, 0);
            } else {
                // todo: rolling buffer somehow needs to be read in the right position even though grains assume a static buffer
                out = self
                    .grain_player
                    .tick(&self.rolling_buffer, self.rolling_offset);
            }
        } else {
            // not looping
        }
        out
    }

    fn tick_static_buffer(&mut self) {
        // don't tick it if its full and we're using it
        if self.use_static_buffer {
            return;
        }
        // fill the static buffer with the loop region
        // we do this by reading the rolling buffer at a delay of the loopable region
        self.static_buffer
            .tick(self.rolling_buffer.read(LOOPABLE_REGION_LENGTH));
        self.rolling_offset += 1;

        // when the rolling offset has reached the end of the loopable region
        // we switch to the static buffer
        if self.rolling_offset >= LOOPABLE_REGION_LENGTH {
            self.use_static_buffer = true;
        }
    }
}
