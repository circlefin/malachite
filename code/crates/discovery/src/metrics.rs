use std::time::{Duration, Instant};

use malachite_metrics::prometheus::metrics::counter::Counter;
use tracing::info;

use malachite_metrics::Registry;

#[derive(Debug)]
pub struct Metrics {
    /// Total number of times we dialed a peer.
    total_dialed: Counter,
    // Total number of times we failed to dial a peer.
    total_failed: Counter,
    /// Time at which discovery started
    start_time: Instant,
    /// Whether we have reached the first idle state
    reached_first_idle: bool,
}

impl Metrics {
    pub fn new(registry: &mut Registry) -> Self {
        let this = Self {
            total_dialed: Counter::default(),
            total_failed: Counter::default(),
            start_time: Instant::now(),
            reached_first_idle: false,
        };

        registry.register(
            "total_dialed",
            "Total number of times we dialed a peer",
            this.total_dialed.clone(),
        );

        registry.register(
            "total_failed",
            "Total number of times we failed to dial a peer",
            this.total_failed.clone(),
        );

        this
    }

    pub fn increment_dial(&mut self) {
        self.total_dialed.inc();
    }

    pub fn increment_failure(&mut self) {
        self.total_failed.inc();
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn register_idle(&mut self, num_peers: usize) {
        if !self.reached_first_idle {
            let total_dialed = self.total_dialed.get();
            let total_failed = self.total_failed.get();

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
