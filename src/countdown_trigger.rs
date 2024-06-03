struct CountdownTrigger {
    count: i32,
}

#[allow(dead_code)]
impl CountdownTrigger {
    pub fn new(count: i32) -> CountdownTrigger {
        CountdownTrigger { count }
    }

    pub fn reset(&mut self, count: i32) {
        self.count = count;
    }

    pub fn tick(&mut self) -> Option<()> {
        if self.count == 0 {
            return Some(());
        } else {
            self.count -= 1;
        }
        return None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_countdown_trigger_none() {
        let mut trigger = CountdownTrigger::new(3);
        trigger.tick();
        trigger.tick();
        trigger.tick();
        trigger.tick();
    }

    #[test]
    fn test_countdown_trigger() {
        let mut trigger = CountdownTrigger::new(3);
        for _i in 0..3 {
            assert_eq!(trigger.tick(), None);
        }
        assert_eq!(trigger.tick(), Some(()));
    }
}
