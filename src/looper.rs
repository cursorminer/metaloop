// looper. simple looper that just loops over a certain segment of the delay line
// the delay line is assumed to not be being ticked, so the data is stationary
use crate::delay_line::DelayLine;

pub struct Looper {
    delay_line: DelayLine,
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
        }
    }

    pub fn tick(&mut self, input: f32) -> f32 {
        self.delay_line.tick(input);
        let out = self.delay_line.read(0);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looper() {
        let mut looper = Looper::new();
        let result = looper.tick(42.0);
        assert_eq!(result, 42.0);
    }
}