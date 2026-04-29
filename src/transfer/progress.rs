use std::time::{Duration, Instant};

pub struct ProgressThrottle {
    interval: Duration,
    last_emit: Instant,
}

impl ProgressThrottle {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_emit: Instant::now() - interval,
        }
    }

    pub fn should_emit(&mut self, force: bool) -> bool {
        if force || self.last_emit.elapsed() >= self.interval {
            self.last_emit = Instant::now();
            return true;
        }
        false
    }
}