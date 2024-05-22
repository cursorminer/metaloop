use crate::delay_line::DelayLine;
use crate::grain::Grain;
use crate::scheduled_grain::ScheduledGrain;

struct GrainPlayer<'a> {
    sample_rate: f32,
    grains: [Grain<'a>; 4],
    delay_line: DelayLine,
}

// schedule and play grains
#[allow(dead_code)]
impl<'a> GrainPlayer<'a> {
    pub fn new(sample_rate: f32) -> GrainPlayer<'a> {
        GrainPlayer {}
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
