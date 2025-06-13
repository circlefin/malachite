use std::time::Duration;

use malachitebft_peer::PeerId;

use super::{Score, ScoringStrategy, SyncResult};

/// Exponential Moving Average scoring strategy
#[derive(Copy, Clone, Debug)]
pub struct ExponentialMovingAverage {
    /// Learning rate for successful responses
    pub alpha_success: f64,

    /// Learning rate for timeouts
    pub alpha_timeout: f64,

    /// Learning rate for failures
    pub alpha_failure: f64,

    /// Threshold for slow responses.
    ///
    /// This should typically be smaller than both the expected
    /// block time and the sync request timeout, as we do not
    /// want responses that are slower than the expected block
    /// time to be considered successful otherwise a node might
    /// not be able to keep up with the network.
    pub slow_threshold: Duration,
}

impl Default for ExponentialMovingAverage {
    fn default() -> Self {
        Self::new(
            0.2,                    // Success
            0.1,                    // Timeout
            0.15,                   // Failure
            Duration::from_secs(1), // Slow threshold
        )
    }
}

impl ExponentialMovingAverage {
    pub fn new(
        alpha_success: f64,
        alpha_timeout: f64,
        alpha_failure: f64,
        slow_threshold: Duration,
    ) -> Self {
        assert!(
            (0.0..=1.0).contains(&alpha_success),
            "alpha_success must be between 0.0 and 1.0"
        );
        assert!(
            (0.0..=1.0).contains(&alpha_timeout),
            "alpha_timeout must be between 0.0 and 1.0"
        );
        assert!(
            (0.0..=1.0).contains(&alpha_failure),
            "alpha_failure must be between 0.0 and 1.0"
        );
        assert!(
            slow_threshold.as_secs_f64() > 0.0,
            "slow_threshold must be greater than zero"
        );

        Self {
            alpha_success,
            alpha_timeout,
            alpha_failure,
            slow_threshold,
        }
    }
}

impl ScoringStrategy for ExponentialMovingAverage {
    fn initial_score(&self, _peer_id: PeerId) -> Score {
        0.5 // All peers start with a neutral score of 0.5
    }

    fn update_score(&mut self, previous_score: Score, result: SyncResult) -> Score {
        match result {
            SyncResult::Success(response_time) => {
                // Calculate quality score between 0-1 based on response time
                // We use an exponential decay function to penalize slow responses more heavily
                let quality = if response_time < self.slow_threshold {
                    // Fast responses get a high quality score
                    1.0
                } else {
                    // Slow responses get a low quality score based on how slow they were
                    (-response_time.as_secs_f64() / self.slow_threshold.as_secs_f64()).exp()
                };

                // Update score with EMA using alpha_success
                self.alpha_success * quality + (1.0 - self.alpha_success) * previous_score
            }

            SyncResult::Timeout => {
                // For timeouts, apply a separate learning rate
                (1.0 - self.alpha_timeout) * previous_score
            }

            SyncResult::Failure => {
                // For failures, apply the failure learning rate
                // This is typically the most severe penalty
                (1.0 - self.alpha_failure) * previous_score
            }
        }
    }
}
