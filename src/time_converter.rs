// centralizes conversions between beats and samples,
// given a sample rate and tempo
pub struct TimeConverter {
    sample_rate: f32,
    tempo: f32,
}

impl TimeConverter {
    pub fn new(sample_rate: f32, tempo: f32) -> TimeConverter {
        TimeConverter { sample_rate, tempo }
    }

    pub fn tempo(&self) -> f32 {
        self.tempo
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo;
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn samples_to_beats(&self, samples: usize) -> f32 {
        samples as f32 / self.sample_rate * self.tempo / 60.0
    }

    pub fn beats_to_samples(&self, beats: f32) -> f32 {
        beats * 60.0 / self.tempo * self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beats_to_samples() {
        let time = TimeConverter::new(10.0, 120.0);
        assert_eq!(time.beats_to_samples(1.0), 5.0);
        assert_eq!(time.beats_to_samples(0.5), 2.5);

        let time2 = TimeConverter::new(10.0, 60.0);
        assert_eq!(time2.beats_to_samples(0.1), 1.0);
    }
}
