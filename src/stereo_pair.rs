use num_traits::Float;
use std::ops::{Add, AddAssign, Mul, Sub};

// the trait that a sample needs in order to be used as an audio grain
pub trait AudioSampleOps:
    Copy
    + Default
    + Add<Self, Output = Self>
    + Sub<Self, Output = Self>
    + Mul<Self, Output = Self>
    + Mul<f32, Output = Self>
    + AddAssign<Self>
{
}

impl<
        T: Copy
            + Default
            + Add<Self, Output = Self>
            + Sub<Self, Output = Self>
            + Mul<Self, Output = Self>
            + Mul<f32, Output = Self>
            + AddAssign<Self>,
    > AudioSampleOps for T
{
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StereoPair<T: Float> {
    pub left: T,
    pub right: T,
}

#[allow(dead_code)]
impl<T: Float> StereoPair<T> {
    pub fn new(left: T, right: T) -> StereoPair<T> {
        StereoPair { left, right }
    }

    pub fn left(&self) -> T {
        self.left
    }

    pub fn right(&self) -> T {
        self.right
    }
}

impl<T: Float> Add for StereoPair<T> {
    type Output = Self;

    fn add(self, other: StereoPair<T>) -> StereoPair<T> {
        StereoPair {
            left: self.left + other.left,
            right: self.right + other.right,
        }
    }
}

impl<T: Float> Sub for StereoPair<T> {
    type Output = Self;

    fn sub(self, other: StereoPair<T>) -> StereoPair<T> {
        StereoPair {
            left: self.left - other.left,
            right: self.right - other.right,
        }
    }
}

impl<T: Float> Mul for StereoPair<T> {
    type Output = Self;

    fn mul(self, other: StereoPair<T>) -> StereoPair<T> {
        StereoPair {
            left: self.left * other.left,
            right: self.right * other.right,
        }
    }
}

impl<T: Float> Mul<T> for StereoPair<T> {
    type Output = Self;
    fn mul(self, scalar: T) -> StereoPair<T> {
        StereoPair {
            left: self.left * scalar,
            right: self.right * scalar,
        }
    }
}

impl<T: Float + std::ops::AddAssign> AddAssign for StereoPair<T> {
    fn add_assign(&mut self, other: StereoPair<T>) {
        self.left += other.left;
        self.right += other.right;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stereo_pair() {
        let pair: StereoPair<f32> = StereoPair::new(1.0, 2.0);
        assert_eq!(pair.left, 1.0);
        assert_eq!(pair.right, 2.0);

        assert_eq!(pair * 2.0 + pair, StereoPair::new(3.0, 6.0));
    }
}
