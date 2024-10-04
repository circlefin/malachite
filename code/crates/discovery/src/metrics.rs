use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct Metrics {
    total_dialed: usize,
    total_failed: usize,
    start_time: Instant,
}

impl Metrics {
    pub(crate) fn new() -> Self {
        Metrics {
            total_dialed: 0,
            total_failed: 0,
            start_time: Instant::now(),
        }
    }

    pub(crate) fn increment_dial(&mut self) {
        self.total_dialed += 1;
    }

    pub(crate) fn increment_failure(&mut self) {
        self.total_failed += 1;
    }

    pub(crate) fn total_dialed(&self) -> usize {
        self.total_dialed
    }

    pub(crate) fn total_failed(&self) -> usize {
        self.total_failed
    }

    pub(crate) fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}
