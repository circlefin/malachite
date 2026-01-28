use std::time::Duration;

use crate::scoring::Strategy;

const DEFAULT_PARALLEL_REQUESTS: u64 = 5;
const DEFAULT_BATCH_SIZE: usize = 5;

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub enabled: bool,
    pub request_timeout: Duration,
    pub max_request_size: usize,
    pub max_response_size: usize,
    pub parallel_requests: u64,
    pub scoring_strategy: Strategy,
    pub inactive_threshold: Option<Duration>,
    pub batch_size: usize,
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

    pub fn with_parallel_requests(mut self, parallel_requests: u64) -> Self {
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
        }
    }
}
