use std::time::{Duration, Instant};

use tracing::info;

#[derive(Debug)]
pub(crate) struct Metrics {
    total_dialed: usize,
    total_failed: usize,
    start_time: Instant,
    reached_first_idle: bool,
}

impl Metrics {
    pub(crate) fn new() -> Self {
        Metrics {
            total_dialed: 0,
            total_failed: 0,
            start_time: Instant::now(),
            reached_first_idle: false,
        }
    }

    pub(crate) fn increment_dial(&mut self) {
        self.total_dialed += 1;
    }

    pub(crate) fn increment_failure(&mut self) {
        self.total_failed += 1;
    }

    pub(crate) fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub(crate) fn register_idle(&mut self, num_peers: usize) {
        if !self.reached_first_idle {
            let total_dialed = self.total_dialed;
            let total_failed = self.total_failed;

            info!(
                "Discovery finished in {}ms, found {} peers, dialed {} peers, {} successful, {} failed",
                self.start_time.elapsed().as_millis(),
                num_peers,
                total_dialed,
                total_dialed - total_failed,
                total_failed,
            );

            self.reached_first_idle = true;
        }
    }
}
