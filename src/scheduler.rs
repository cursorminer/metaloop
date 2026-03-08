use arrayvec::ArrayVec;
use std::collections::VecDeque;

/// Maximum number of events that can fire on a single tick.
/// In practice this is ~3-4, but 8 gives comfortable headroom.
pub const MAX_EVENTS_PER_TICK: usize = 8;

// E is the event type
pub struct Scheduler<E: Clone + Copy + PartialEq> {
    events: VecDeque<(f32, E)>,
}

#[allow(dead_code)]
impl<E: Clone + Copy + PartialEq> Scheduler<E> {
    pub fn new() -> Scheduler<E> {
        Scheduler {
            events: VecDeque::with_capacity(100),
        }
    }

    pub fn reset(&mut self) {
        self.clear();
    }

    pub fn schedule_event(&mut self, new_event_time: f32, event: E) {
        let previous_event_time = self.events.back().map(|&(t, _)| t).unwrap_or(0.0);

        if new_event_time < previous_event_time {
            eprintln!(
                "event must be scheduled after previous event, new_event_time: {}, previous_event_time: {}",
                new_event_time, previous_event_time,
            );
            return;
        }
        self.events.push_back((new_event_time, event));
    }

    pub fn tick(&mut self, time: f32) -> ArrayVec<E, MAX_EVENTS_PER_TICK> {
        let mut events = ArrayVec::new();
        while let Some(&(event_time, ref event)) = self.events.front() {
            if event_time <= time {
                events.push(event.clone());
                self.events.pop_front();
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
        assert_eq!(scheduler.tick(0.0).as_slice(), &[]);
        assert_eq!(scheduler.tick(1.0).as_slice(), &[TestEvent::A]);
        assert_eq!(scheduler.tick(1.5).as_slice(), &[]);
        assert_eq!(scheduler.tick(2.0).as_slice(), &[TestEvent::B]);
        assert_eq!(
            scheduler.tick(4.0).as_slice(),
            &[TestEvent::A, TestEvent::B]
        );
        assert_eq!(scheduler.tick(4.5).as_slice(), &[]);
        scheduler.clear();
        assert_eq!(scheduler.tick(5.0).as_slice(), &[]);
    }
}
