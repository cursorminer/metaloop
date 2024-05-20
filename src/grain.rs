use crate::delay_line::DelayLine;

// a rather short lived thing that plays a single faded grain
pub struct Grain<'a> {
    buffer: &'a DelayLine, 
    offset_counter: usize,
    duration_samples: usize,
    fade_duration: usize,
}

#[allow(dead_code)]
impl<'a> Grain<'a> {      

    pub fn new(buf: &'a DelayLine, offset: usize, duration: usize, fade: usize) -> Grain<'a> {
        Grain{buffer: buf, offset_counter: offset, duration_samples: duration, fade_duration: fade}
    }

    pub fn tick(&mut self) -> f32 {
        if self.is_finished()
        {
            return 0.0;
        }

        self.offset_counter = self.offset_counter - 1;
        // get window amplitude
        let win = 1.0;
        // read buffer
        win * self.buffer.read(self.offset_counter)
    }

    pub fn is_finished(& self) -> bool {
        return self.offset_counter == 0;
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_delay_ramp(delay_line: &mut DelayLine)
    {
        for i in 0..delay_line.len() {
            delay_line.tick(i as f32);
        }
    }

    #[test]
    fn test_grain()
    {

        let mut delay_line = DelayLine::new(20);
        fill_delay_ramp(&mut delay_line);

        let mut grain = Grain::new(&delay_line, 10, 5, 0);
        assert_eq!(grain.tick(), 10.0);
    }
}