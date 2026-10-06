use std::cmp::max;
use std::time::Duration;

use crate::scoring::Strategy;

const DEFAULT_PARALLEL_REQUESTS: usize = 5;
const DEFAULT_BATCH_SIZE: usize = 5;
const DEFAULT_INBOUND_REQUEST_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(10);
const DEFAULT_MAX_INBOUND_REQUESTS_PER_WINDOW: u32 = 1000;
const DEFAULT_MAX_CONCURRENT_INBOUND_REQUESTS: usize = 32;
const DEFAULT_MAX_PENDING_INBOUND_REQUESTS: usize = 64;

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub enabled: bool,
    pub request_timeout: Duration,
    pub max_request_size: usize,
    pub max_response_size: usize,
    pub parallel_requests: usize,
    pub scoring_strategy: Strategy,
    pub inactive_threshold: Option<Duration>,
    pub batch_size: usize,
    /// Rolling window over which inbound value requests from each peer are counted.
    pub inbound_request_rate_limit_window: Duration,
    /// Maximum inbound value requests accepted per peer within each window.
    pub max_inbound_requests_per_window: u32,
    /// Maximum number of inbound value requests processed concurrently against the host.
    pub max_concurrent_inbound_requests: usize,
    /// Maximum number of inbound value requests that may wait for a concurrent
    /// slot once the concurrent cap is reached. Excess requests past this
    /// capacity are rejected outright with an empty response.
    pub max_pending_inbound_requests: usize,
}

impl Config {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }

    pub fn with_request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = request_timeout;
        self
    }

    pub fn with_max_request_size(mut self, max_request_size: usize) -> Self {
        self.max_request_size = max_request_size;
        self
    }

    pub fn with_max_response_size(mut self, max_response_size: usize) -> Self {
        self.max_response_size = max_response_size;
        self
    }

    pub fn with_parallel_requests(mut self, parallel_requests: usize) -> Self {
        self.parallel_requests = parallel_requests;
        self
    }

    pub fn with_scoring_strategy(mut self, scoring_strategy: Strategy) -> Self {
        self.scoring_strategy = scoring_strategy;
        self
    }

    pub fn with_inactive_threshold(mut self, inactive_threshold: Option<Duration>) -> Self {
        self.inactive_threshold = inactive_threshold;
        self
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub fn with_inbound_request_rate_limit_window(mut self, window: Duration) -> Self {
        self.inbound_request_rate_limit_window = window;
        self
    }

    pub fn with_max_inbound_requests_per_window(mut self, max_requests: u32) -> Self {
        self.max_inbound_requests_per_window = max_requests;
        self
    }

    pub fn with_max_concurrent_inbound_requests(mut self, max_concurrent: usize) -> Self {
        self.max_concurrent_inbound_requests = max_concurrent;
        self
    }

    pub fn with_max_pending_inbound_requests(mut self, max_pending: usize) -> Self {
        self.max_pending_inbound_requests = max_pending;
        self
    }

    /// The parallel-request limit used by ValueSync.
    pub fn effective_parallel_requests(&self) -> usize {
        max(1, self.parallel_requests)
    }

    /// The batch-size limit used by ValueSync.
    pub fn effective_batch_size(&self) -> usize {
        max(1, self.batch_size)
    }

    /// The distance from the tip to the highest permitted request start.
    pub fn read_ahead_window(&self) -> usize {
        self.effective_parallel_requests()
            .saturating_mul(self.effective_batch_size())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            request_timeout: Duration::from_secs(10),
            max_request_size: 1024 * 1024,       // 1 MiB
            max_response_size: 10 * 1024 * 1024, // 10 MiB
            parallel_requests: DEFAULT_PARALLEL_REQUESTS,
            scoring_strategy: Strategy::default(),
            inactive_threshold: None,
            batch_size: DEFAULT_BATCH_SIZE,
            inbound_request_rate_limit_window: DEFAULT_INBOUND_REQUEST_RATE_LIMIT_WINDOW,
            max_inbound_requests_per_window: DEFAULT_MAX_INBOUND_REQUESTS_PER_WINDOW,
            max_concurrent_inbound_requests: DEFAULT_MAX_CONCURRENT_INBOUND_REQUESTS,
            max_pending_inbound_requests: DEFAULT_MAX_PENDING_INBOUND_REQUESTS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn zero_request_limits_default_to_one() {
        let config = Config::default()
            .with_parallel_requests(0)
            .with_batch_size(0);

        assert_eq!(config.effective_parallel_requests(), 1);
        assert_eq!(config.effective_batch_size(), 1);
    }
}
