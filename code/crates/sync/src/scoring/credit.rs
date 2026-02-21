use std::collections::{HashMap, HashSet};
use std::time::Duration;

use malachitebft_peer::PeerId;

use super::{Score, ScoringStrategy, SyncResult};

#[derive(Copy, Clone, Debug)]
pub struct CreditConfig {
    /// Threshold for what is considered "fast enough".
    pub slow_threshold: Duration,
    /// Credit deltas
    pub credit_fast_success: i32,
    /// Credit delta for a success that's slower than the slow_threshold.
    pub credit_slow_success: i32,
    /// Credit delta for a failure.
    pub credit_failure: i32,
    /// Credit delta for a timeout.
    pub credit_timeout: i32,
    /// Minimum credit (worst score).
    pub min_credit: i32,
    /// Maximum credit (best score).
    pub max_credit: i32,
}

impl Default for CreditConfig {
    fn default() -> Self {
        CreditConfig {
            slow_threshold: Duration::from_millis(500),
            credit_fast_success: 2,
            credit_slow_success: 0,
            credit_failure: -2,
            credit_timeout: -4,
            min_credit: -20,
            max_credit: 20,
        }
    }
}

/// Credit-based scoring strategy
///
/// Maintain an integer "credit" per peer.
/// - Fast success increases credit more than slow success.
/// - Failures and timeouts reduce credit.
///
/// Credits are clamped to [min_credit, max_credit].
/// Score is a normalized mapping of credit -> [0.0, 1.0].
#[derive(Clone, Debug)]
pub struct Credit {
    config: CreditConfig,

    /// Per-peer credits
    credits: HashMap<PeerId, i32>,
}

impl Default for Credit {
    fn default() -> Self {
        Self::new(CreditConfig::default())
    }
}

impl Credit {
    pub fn new(config: CreditConfig) -> Self {
        assert!(
            config.slow_threshold.as_secs_f64() > 0.0,
            "slow_threshold must be > 0"
        );

        assert!(
            config.min_credit < config.max_credit,
            "min_credit must be < max_credit"
        );

        Self {
            config,
            credits: HashMap::new(),
        }
    }

    fn clamp_credit(&self, c: i32) -> i32 {
        c.clamp(self.config.min_credit, self.config.max_credit)
    }

    /// Map credit in [min_credit, max_credit] to score in [0.0, 1.0].
    fn credit_to_score(&self, credit: i32) -> Score {
        let min = self.config.min_credit as f64;
        let max = self.config.max_credit as f64;
        let c = credit as f64;

        // Avoid division by zero if min and max are the same
        // (though this should be prevented by the constructor).
        if (max - min).abs() < f64::EPSILON {
            return 0.5;
        }

        ((c - min) / (max - min)).clamp(0.0, 1.0)
    }

    fn initial_credit(&self) -> i32 {
        // Neutral: midpoint of the clamp range.
        self.config.min_credit + (self.config.max_credit - self.config.min_credit) / 2
    }

    fn get_credit(&mut self, peer_id: PeerId) -> i32 {
        let init = self.initial_credit();
        *self.credits.entry(peer_id).or_insert(init)
    }

    fn set_credit(&mut self, peer_id: PeerId, credit: i32) -> i32 {
        let new_credit = self.clamp_credit(credit);
        self.credits.insert(peer_id, new_credit);
        new_credit
    }

    fn is_fast(&self, response_time: Duration) -> bool {
        response_time < self.config.slow_threshold
    }
}

impl ScoringStrategy for Credit {
    fn initial_score(&self, _peer_id: PeerId) -> Score {
        // Note: this returns a neutral score, but actual per-peer credit is created on first update.
        self.credit_to_score(self.initial_credit())
    }

    fn update_score(
        &mut self,
        peer_id: PeerId,
        _previous_score: Score,
        result: SyncResult,
    ) -> Score {
        let credit = self.get_credit(peer_id);

        let delta = match result {
            SyncResult::Success(rt) => {
                if self.is_fast(rt) {
                    self.config.credit_fast_success
                } else {
                    self.config.credit_slow_success
                }
            }
            SyncResult::Failure => self.config.credit_failure,
            SyncResult::Timeout => self.config.credit_timeout,
        };

        let new_credit = self.set_credit(peer_id, credit.saturating_add(delta));

        eprintln!(
            "result={result:?}, credit={credit}, delta={delta}, new={}, score={:.2}",
            new_credit,
            self.credit_to_score(new_credit)
        );

        self.credit_to_score(new_credit)
    }

    fn is_stateful(&self) -> bool {
        true
    }

    fn retain_only(&mut self, peer_ids: HashSet<&PeerId>) {
        self.credits.retain(|peer_id, _| peer_ids.contains(peer_id));
    }
}
