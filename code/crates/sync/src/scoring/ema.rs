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

    /// Quality score in `[0.0, 1.0]` derived from response time alone.
    /// Fast responses (at or below `slow_threshold`) score 1.0; slower ones
    /// decay exponentially from the threshold.
    fn response_quality(&self, response_time: Duration) -> f64 {
        let response_time_secs = response_time.as_secs_f64();
        let threshold_secs = self.slow_threshold.as_secs_f64();

        if response_time_secs <= threshold_secs {
            1.0
        } else {
            (-(response_time_secs - threshold_secs) / threshold_secs).exp()
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
                let quality = self.response_quality(response_time);

                // Update score with EMA using alpha_success
                let new_score =
                    self.alpha_success * quality + (1.0 - self.alpha_success) * previous_score;

                #[cfg(test)]
                {
                    let response_time_secs = response_time.as_secs_f64();
                    println!("Response time: {response_time_secs:.3}s, Quality: {quality:.3}");
                    println!(" => Updating score: prev={previous_score:.3}, new={new_score:.3}");
                }

                new_score
            }

            SyncResult::PartialSuccess {
                received,
                requested,
                response_time,
            } => {
                // Scale the response-time quality by the fraction of the requested
                // range that was delivered. A full-ratio partial response scores
                // identically to `Success`; a ratio of 0 collapses the quality
                // contribution to zero and pulls the score toward 0.0 at least
                // as hard as `Failure` does (since `alpha_success >= alpha_failure`
                // is the common default).
                let ratio = if requested == 0 {
                    0.0
                } else {
                    (received as f64 / requested as f64).clamp(0.0, 1.0)
                };

                let quality = self.response_quality(response_time) * ratio;
                let new_score =
                    self.alpha_success * quality + (1.0 - self.alpha_success) * previous_score;

                #[cfg(test)]
                {
                    let response_time_secs = response_time.as_secs_f64();
                    println!("Partial response ({received}/{requested}) time: {response_time_secs:.3}s, Quality: {quality:.3}");
                    println!(" => Updating score: prev={previous_score:.3}, new={new_score:.3}");
                }

                new_score
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
