use crate::delay_line::DelayLine;
use crate::grain::Grain;

// a grain that can be scheduled to play at a later time
pub struct ScheduledGrain {
    grain: Grain,
    countdown: u64,
}
/*
impl<'a> ScheduledGrain<'a> {
    pub fn new() -> Self {
        let grain = Grain::new(DelayLine::new(0), 0, 0, 0);
        Self {
            grain,
            countdown: scheduled_at,
        }
    }

    pub fn tick(&mut self) -> f32 {
        if self.countdown == 0 {
            return self.grain.tick();
        }
        self.countdown -= 1;
        0.0
    }

    pub fn is_waiting(&self) -> bool {
        self.countdown > 0
    }

    pub fn is_finished(&self) -> bool {
        self.grain.is_finished()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduled_grain() {
        let del = DelayLine::new(20);
        let grain = Grain::new(del, 10, 1, 0);

        let mut scheduled_grain = ScheduledGrain::new(grain, 0);
        assert_eq!(scheduled_grain.is_waiting(), true);
        assert_eq!(scheduled_grain.is_finished(), false);
        assert_eq!(scheduled_grain.tick(), 0.0);
        assert_eq!(scheduled_grain.is_waiting(), false);
        assert_eq!(scheduled_grain.is_finished(), true);
    }
}
*/
