// delay line
use std::ops::{Add, Mul, Sub};

pub struct DelayLine {
    buffer: Vec<f32>,
    write_index: usize,
}

pub fn lerp<T, X>(a: T, b: T, f: X) -> T
where
    T: Mul<X, Output = T> + Add<T, Output = T> + Sub<T, Output = T> + Copy,
    X: Copy + Mul<T, Output = T>,
{
    f * (b - a) + a
}

pub fn fill_delay_ramp(delay_line: &mut DelayLine) {
    for i in 0..delay_line.len() {
        delay_line.tick(i as f32);
    }
}

pub fn fill_delay_constant(delay_line: &mut DelayLine, value: f32) {
    for _i in 0..delay_line.len() {
        delay_line.tick(value);
    }
}

#[allow(dead_code)]
impl DelayLine {
    pub fn new(size: usize) -> DelayLine {
        DelayLine {
            buffer: vec![0.0; size],
            write_index: 0,
        }
    }

    pub fn reset(&mut self) {
        self.write_index = 0;
    }

    pub fn tick(&mut self, value: f32) {
        self.buffer[self.write_index] = value;
        self.write_index = (self.write_index + 1) % self.buffer.len();
    }

    pub fn read(&self, delay_samples: usize) -> f32 {
        assert!(
            delay_samples < self.buffer.len(),
            "delay was: {:?}",
            delay_samples
        );

        let read_index =
            (self.write_index + self.buffer.len() - delay_samples - 1) % self.buffer.len();
        let value = self.buffer[read_index];
        value
    }

    pub fn read_interpolated(&self, delay_samples: f32) -> f32 {
        let i0 = delay_samples.floor() as usize;
        let i1 = delay_samples.ceil() as usize;
        assert!(i1 < self.buffer.len());
        let frac = delay_samples - i0 as f32;

        let v0 = self.read(i0);
        let v1 = self.read(i1);

        lerp(v0, v1, frac)
    }

    pub fn len(&self) -> usize {
        return self.buffer.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lerp() {
        assert_eq!(lerp(0.0, 1.0, 0.4), 0.4);
        assert_eq!(lerp(0.0, 10.0, 0.9), 9.0);
    }

    #[test]
    fn test_delay_line() {
        let mut delay_line = DelayLine::new(4);
        delay_line.reset();
        assert_eq!(delay_line.read(3), 0.0);
        delay_line.tick(1.0);
        delay_line.tick(2.0);
        delay_line.tick(3.0);
        delay_line.tick(4.0);
        assert_eq!(delay_line.read(0), 4.0);
        assert_eq!(delay_line.read(1), 3.0);
        assert_eq!(delay_line.read(2), 2.0);
        assert_eq!(delay_line.read(3), 1.0);

        assert_eq!(delay_line.read_interpolated(0.6), 3.4);
    }
}
