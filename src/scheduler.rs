// E is the event type
pub struct Scheduler<E: Clone + Copy + PartialEq + Eq> {
    events: Vec<(f32, E)>,
}

#[allow(dead_code)]
impl<E: Clone + Copy + PartialEq + Eq> Scheduler<E> {
    pub fn new() -> Scheduler<E> {
        Scheduler { events: Vec::new() }
    }

    pub fn schedule_event(&mut self, time: f32, event: E) {
        assert!(time >= self.events.last().map(|&(t, _)| t).unwrap_or(0.0));
        self.events.push((time, event));
    }

    pub fn tick(&mut self, time: f32) -> Vec<E> {
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]

    pub enum TestEvent {
        A,
        B,
    }

    #[test]
    fn test_scheduler() {
        let mut scheduler = Scheduler::<TestEvent>::new();
        scheduler.schedule_event(1.0, TestEvent::A);
        scheduler.schedule_event(2.0, TestEvent::B);
        scheduler.schedule_event(3.0, TestEvent::A);
        scheduler.schedule_event(4.0, TestEvent::B);
        scheduler.schedule_event(5.0, TestEvent::A);
        assert_eq!(scheduler.tick(0.0), vec![]);
        assert_eq!(scheduler.tick(1.0), vec![TestEvent::A]);
        assert_eq!(scheduler.tick(1.5), vec![]);
        assert_eq!(scheduler.tick(2.0), vec![TestEvent::B]);
        assert_eq!(scheduler.tick(4.0), vec![TestEvent::A, TestEvent::B]);
        assert_eq!(scheduler.tick(4.5), vec![]);
        scheduler.clear();
        assert_eq!(scheduler.tick(5.0), vec![]);
    }
}
