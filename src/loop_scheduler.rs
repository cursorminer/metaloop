// This handles the actual events that control what the looper does
pub struct LoopScheduler {}

impl LoopScheduler {
    pub fn new() -> LoopScheduler {
        LoopScheduler {}
    }

    pub fn start_looping(&mut self, grid_interval: f32) {
        // schedule a fade out
        // schedule a grain to start at the next grid interval
    }

    pub fn tick(&mut self, beat_time: f32) {
        // Do nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_scheduler() {
        let mut scheduler = LoopScheduler::new();
        scheduler.tick(0.0);
    }
}
