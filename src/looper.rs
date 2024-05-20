
use crate::delay_line::DelayLine;

// looper. simple looper that just loops over a certain segment of the delay line
// the delay line is assumed to not be being ticked, so the data is stationary
// the subtleties of where exactly the loop should be are up to the client
pub struct Looper {
    delay_line: DelayLine,
    
    is_looping: bool,

    loop_start: usize,
    loop_end: usize,

    fade_loop_start: usize,
    fade_loop_end: usize,
    
    current_read_position: usize,
    fading_read_position: usize,
    fade_length_samples: usize,
}

// wraps an unsigned integer into a given range [min, max]
pub fn wrap(i: usize, min: usize, max: usize) -> usize {
    assert!(min < max);
    if max == 0 {
        return 0;
    }
    let range = max - min + 1;
    if i >= min
    {
     return ((i - min) % range) + min;
    }
    else {
        let offset = min % range;
        let i_offset = i % range;
        return (((i_offset + range) - offset) % range) + min;
    }
}

#[allow(dead_code)]
impl Looper
{
    pub fn new() -> Self
    { 
        let size = 64;
        let del = DelayLine::new(size);
        Self{
            delay_line: del,
            is_looping: false,
            loop_start: 10,
            loop_end: 0,
            fade_loop_start: 10,
            fade_loop_end: 0,
            current_read_position: 0,
            fading_read_position: 0,
            fade_length_samples: 0,
        }
    }

    // set the start and end position of the loop, indexed in samples counting back from the most recently input sample
    pub fn set_looping_region(&mut self,  start: usize, end: usize) { 
        // note that since pos is a delay, the start is larger than the end
        self.is_looping = true;
        assert!(start > end);
        self.loop_start = start;
        self.loop_end = end;
        // start at the start of the loop
        self.current_read_position = self.loop_start;

        // set up fading position
        self.fade_loop_end = self.loop_end + self.fade_length_samples;
        self.fade_loop_start = self.loop_start + self.fade_length_samples;
    }

    pub fn set_fade_length(&mut self, length_samples: usize){
        self.fade_length_samples = std::cmp::min(length_samples, self.loop_length());
    }

    fn loop_length(&self) -> usize {
        return self.loop_end - self.loop_start + 1;
    }

    pub fn stop_looping(&mut self)
    {
        self.is_looping = false;
    }

    pub fn tick_delay(&mut self, input: f32) {
        assert!(!self.is_looping);
        self.delay_line.tick(input);
    }

    pub fn tick_loop(&mut self) -> f32 {
        if !self.is_looping {
            return self.delay_line.read(0);
        }
        
        let out = self.delay_line.read(self.current_read_position);

        self.tick_read_pos();
        out
    }

    fn tick_read_pos(&mut self) -> usize {
        self.current_read_position = wrap(self.current_read_position - 1, self.loop_end, self.loop_start);
        // self.fading_read_position = wrap(self.fading_read_position - 1, self.fade_loop_end, self.fade_loop_start);
        return self.current_read_position;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap()
    {
        assert_eq!(wrap(2, 2, 5), 2);
        assert_eq!(wrap(5, 2, 5), 5);
        assert_eq!(wrap(6, 2, 5), 2);
        assert_eq!(wrap(1, 2, 5), 5);

       assert_eq!( wrap(10, 0, 30), 10);
       assert_eq!(wrap(30, 0, 30), 30);

       assert_eq!(wrap(10, 20, 30), 21);
       assert_eq!(wrap(105, 10, 20), 17);

    }

    #[test]
    fn test_looper_dry() {
        let mut looper = Looper::new();
        looper.stop_looping();
        looper.tick_delay(42.0);
        let result = looper.tick_loop();
        assert_eq!(result, 42.0);
    }

    #[test]
    fn test_looper_readpos() {
        let mut looper = Looper::new();
        looper.set_looping_region(8, 4);
        let mut out = vec![];
        for _i in 0..10 {
            out.push(looper.tick_read_pos());
        }
        let expected = vec![7, 6, 5, 4, 8, 7, 6, 5, 4, 8];
        assert_eq!(out, expected);
    }

    #[test]
    fn test_looper_loop() {
        let mut looper = Looper::new();

        // put 10 samples into the buffer
        for i in 0..11 {
            looper.tick_delay(i as f32);
        }

        // set the loop region to be the first 6 samples of this buffer
        looper.set_looping_region(10, 5);

        let mut out = vec![];
        for _i in 0..10 {
            out.push(looper.tick_loop());
        }
        let expected = vec!(0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 1.0, 2.0, 3.0);
        assert_eq!(out, expected);
    }


    // TODO
    // #[test]
    // fn test_looper_fade() {
    //     let mut looper = Looper::new();

    //     // first half of buffer is 0
    //     for _i in 0..4 {
    //         looper.tick_delay(0.0);
    //     }
    //     // second half 1
    //     for _i in 0..4 {
    //         looper.tick_delay(1.0);
    //     }

    //     // set the loop region to whole buffer
    //     looper.set_looping_region(8, 0);
    //     looper.set_fade_length(4);

    //     let mut out = vec![];
    //     for _i in 0..10 {
    //         out.push(looper.tick_loop());
    //     }
    //     let expected = vec!(0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 1.0, 2.0, 3.0);
    // }
}