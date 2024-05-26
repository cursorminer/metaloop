use crate::delay_line::lerp;

pub struct RampedValue {
    value: f32,
    target_value: f32,
    initial_value: f32,
    ramp_time_counter: usize,
    ramp_time_total: usize,
}

impl RampedValue {
    pub fn new(initial_value: f32) -> RampedValue {
        RampedValue {
            value: initial_value,
            target_value: initial_value,
            initial_value: initial_value,
            ramp_time_counter: 0,
            ramp_time_total: 0,
        }
    }

    pub fn set(&mut self, value: f32) {
        self.value = value;
        self.initial_value = self.value;
        self.ramp_time_counter = 0;
    }

    // ramp duration is in samples spent at intermediate values, so target
    // is reached after ramp_time + 1 samples and the ramp moves away from initial value
    // immediately
    pub fn ramp(&mut self, target_value: f32, ramp_time: usize) {
        self.ramp_time_counter = ramp_time + 1;
        self.ramp_time_total = ramp_time + 1;
        self.target_value = target_value;
    }

    pub fn tick(&mut self) -> f32 {
        if self.ramp_time_counter == 0 {
            return self.target_value;
        }
        self.ramp_time_counter -= 1;
        let frac = self.ramp_time_counter as f32 / self.ramp_time_total as f32;
        self.value = lerp(self.initial_value, self.target_value, 1.0 - frac);
        self.value
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;

    use super::*;

    const EPS: f32 = 0.0001;

    #[test]
    fn test_ramped_value() {
        let mut ramped_value = RampedValue::new(0.0);

        ramped_value.set(0.0);
        assert_eq!(ramped_value.tick(), 0.0);

        ramped_value.ramp(1.0, 4);
        assert_abs_diff_eq!(ramped_value.tick(), 0.2, epsilon = EPS);
        assert_abs_diff_eq!(ramped_value.tick(), 0.4, epsilon = EPS);
        assert_abs_diff_eq!(ramped_value.tick(), 0.6, epsilon = EPS);
        assert_abs_diff_eq!(ramped_value.tick(), 0.8, epsilon = EPS);
        assert_abs_diff_eq!(ramped_value.tick(), 1.0, epsilon = EPS);
        assert_abs_diff_eq!(ramped_value.tick(), 1.0, epsilon = EPS);
    
        
    }

    #[test]
    fn test_ramped_value_zero_length() {
        let mut ramped_value = RampedValue::new(0.0);

        ramped_value.ramp(1.0, 0);
        assert_eq!(ramped_value.tick(), 1.0);
    }

    #[test]
    fn test_ramped_value_down() {
        let mut ramped_value = RampedValue::new(1.0);

        assert_eq!(ramped_value.tick(), 1.0);

        ramped_value.ramp(0.0, 3);
        assert_eq!(ramped_value.tick(), 0.75);
        assert_eq!(ramped_value.tick(), 0.5);
        assert_eq!(ramped_value.tick(), 0.25);
        assert_eq!(ramped_value.tick(), 0.0);
    }
}
