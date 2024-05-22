use crate::delay_line::DelayLine;
use crate::grain::Grain;

struct GrainPlayer {
    sample_rate: f32,
}

// schedule and play grains
#[allow(dead_code)]
impl GrainPlayer {
    pub fn new(sample_rate: f32) -> GrainPlayer {
        GrainPlayer { sample_rate }
    }

    pub fn tick(&mut self) -> f32 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grain_player() {
        let mut player = GrainPlayer::new(44100.0);
        let out = player.tick();
        assert_eq!(out, 0.0);
    }
}
