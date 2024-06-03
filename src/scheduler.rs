#[derive(Debug, Clone, Copy)]
pub enum LoopEvent {
    StartGrain,
    StopGrain,
}

pub struct Scheduler {
    events: Vec<(f32, LoopEvent)>,
}

#[allow(dead_code)]
impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler { events: Vec::new() }
    }

    pub fn schedule_event(&mut self, time: f32, event: LoopEvent) {
        assert!(time >= self.events.last().map(|&(t, _)| t).unwrap_or(0.0));
        self.events.push((time, event));
    }

    pub fn tick(&mut self, time: f32) -> Vec<LoopEvent> {
        let mut events = Vec::new();
        while let Some(&(event_time, ref event)) = self.events.first() {
            if event_time <= time {
                events.push(event.clone());
                self.events.remove(0);
            } else {
                break;
            }
        }
        events
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler() {
        let mut scheduler = Scheduler::new();
        scheduler.schedule_event(1.0, LoopEvent::StartGrain);
        scheduler.schedule_event(2.0, LoopEvent::StopGrain);
        scheduler.schedule_event(3.0, LoopEvent::StartGrain);
        scheduler.schedule_event(4.0, LoopEvent::StopGrain);
        scheduler.schedule_event(5.0, LoopEvent::StartGrain);
        assert_eq!(scheduler.tick(0.0), vec![]);
        assert_eq!(scheduler.tick(1.0), vec![LoopEvent::StartGrain]);
        assert_eq!(scheduler.tick(1.5), vec![]);
        assert_eq!(scheduler.tick(2.0), vec![LoopEvent::StopGrain]);
        assert_eq!(
            scheduler.tick(4.0),
            vec![LoopEvent::StartGrain, LoopEvent::StopGrain]
        );
        assert_eq!(scheduler.tick(4.5), vec![]);
        scheduler.clear();
        assert_eq!(scheduler.tick(5.0), vec![]);
    }
}
