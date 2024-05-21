use crate::delay_line::DelayLine;

fn trapezoid_window(pos: usize, duration: usize, fade: usize) -> f32 {
    if fade == 0 {
        return 1.0;
    }

    if pos < fade {
        let frac = (pos) as f32 / fade as f32;
        return frac;
    } else if pos > duration {
        return 0.0;
    } else if pos <= (duration - fade){
        return 1.0;
    } else {
        let frac = ((duration + 1) - pos) as f32 / fade as f32;
        return frac;
    }
}


// a rather short lived thing that plays a single faded grain
// the duration includes two fade durations
pub struct Grain<'a> {
    buffer: &'a DelayLine, 
    delay_pos: usize,
    end_delay: usize,
    duration: usize,
    fade_duration: usize,
    window_pos: usize,
}

#[allow(dead_code)]
impl<'a> Grain<'a> {      

    // offset: the initial delay time where the grain starts
    // duration: how long the grain lasts
    // fade: number of samples to fade in and out (this is within the duration above)
    pub fn new(buf: &'a DelayLine, offset: usize, duration: usize, fade: usize) -> Grain<'a> {
        assert!(duration < buf.len());
        assert!(offset < buf.len());
        assert!(offset >= duration);

        let actual_fade = if (fade * 2) > duration { duration / 2 } else {fade}; 

        Grain{buffer: buf, delay_pos: offset, end_delay: offset - duration, duration: duration, fade_duration: actual_fade, window_pos: 0}
    }

    pub fn tick(&mut self) -> f32 {
        if self.is_finished()
        {
            return 0.0; 
        }

        self.delay_pos = self.delay_pos - 1;
        self.window_pos = self.window_pos + 1; // starts at one so the window is non-zero immediately
        
        let win = trapezoid_window(self.window_pos, self.duration, self.fade_duration);
        let out = self.buffer.read(self.delay_pos);
        
        win * out
    }

    pub fn stop(&mut self) {
        // if already fading out don't stop it
        if self.window_pos > (self.duration - self.fade_duration) {
            return;
        }

        // otherwise tweak the values so that the grain fades now
        self.duration = self.window_pos + self.fade_duration;
        self.end_delay = self.delay_pos + self.fade_duration;
    }

    pub fn is_finished(& self) -> bool {
        return self.delay_pos == self.end_delay;
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

    fn fill_delay_constant(delay_line: &mut DelayLine, value: f32)
    {
        for _i in 0..delay_line.len() {
            delay_line.tick(value);
        }
    }

    #[test]
    fn test_grain()
    {

        let mut delay_line = DelayLine::new(20);
        fill_delay_ramp(&mut delay_line);

        let mut grain = Grain::new(&delay_line, 10, 5, 0);

        let expected = vec![10.0, 11.0, 12.0, 13.0, 14.0, 0.0];
        let mut out = vec![];
        for _i in 0..expected.len() {
            // assert!(!grain.is_finished());
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
        assert!(grain.is_finished());
    }

    #[test]
    fn test_grain_fade()
    {

        let mut delay_line = DelayLine::new(20);
        fill_delay_constant(&mut delay_line, 4.0);

        let mut grain = Grain::new(&delay_line, 10, 9, 4);

        let expected = vec![1.0, 2.0, 3.0, 4.0, 4.0, 4.0, 3.0, 2.0, 1.0, 0.0];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);
    }

    #[test]
    fn test_grain_stop()
    {
        let mut delay_line = DelayLine::new(30);
        fill_delay_constant(&mut delay_line, 4.0);

        let mut grain = Grain::new(&delay_line, 20, 15, 4);

        let expected = vec![1.0, 2.0, 3.0, 4.0, 4.0, 4.0];
        let mut out = vec![];
        for _i in 0..expected.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected);

        let expected_fade = vec![4.0, 3.0, 2.0, 1.0, 0.0];

        grain.stop();
        let mut out = vec![];
        for _i in 0..expected_fade.len() {
            out.push(grain.tick());
        }

        assert_eq!(out, expected_fade);
    }
}