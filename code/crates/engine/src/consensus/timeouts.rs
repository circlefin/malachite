use std::time::Duration;

use malachitebft_core_types::TimeoutKind;

/// Timeouts
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Timeouts {
    /// How long we wait for a proposal block before prevoting nil
    pub timeout_propose: Duration,

    /// How much timeout_propose increases with each round
    pub timeout_propose_delta: Duration,

    /// How long we wait after receiving +2/3 prevotes for “anything” (ie. not a single block or nil)
    pub timeout_prevote: Duration,

    /// How much the timeout_prevote increases with each round
    pub timeout_prevote_delta: Duration,

    /// How long we wait after receiving +2/3 precommits for “anything” (ie. not a single block or nil)
    pub timeout_precommit: Duration,

    /// How much the timeout_precommit increases with each round
    pub timeout_precommit_delta: Duration,

    /// How long we wait after entering a round before starting
    /// the rebroadcast liveness protocol
    pub timeout_rebroadcast: Duration,
}

impl Timeouts {
    pub fn timeout_duration(&self, step: TimeoutKind) -> Duration {
        match step {
            TimeoutKind::Propose => self.timeout_propose,
            TimeoutKind::Prevote => self.timeout_prevote,
            TimeoutKind::Precommit => self.timeout_precommit,
            TimeoutKind::Rebroadcast => {
                self.timeout_propose + self.timeout_prevote + self.timeout_precommit
            }
        }
    }

    pub fn delta_duration(&self, step: TimeoutKind) -> Option<Duration> {
        match step {
            TimeoutKind::Propose => Some(self.timeout_propose_delta),
            TimeoutKind::Prevote => Some(self.timeout_prevote_delta),
            TimeoutKind::Precommit => Some(self.timeout_precommit_delta),
            TimeoutKind::Rebroadcast => None,
        }
    }
}

impl Default for Timeouts {
    fn default() -> Self {
        let timeout_propose = Duration::from_secs(3);
        let timeout_prevote = Duration::from_secs(1);
        let timeout_precommit = Duration::from_secs(1);
        let timeout_rebroadcast = timeout_propose + timeout_prevote + timeout_precommit;

        Self {
            timeout_propose,
            timeout_propose_delta: Duration::from_millis(500),
            timeout_prevote,
            timeout_prevote_delta: Duration::from_millis(500),
            timeout_precommit,
            timeout_precommit_delta: Duration::from_millis(500),
            timeout_rebroadcast,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_durations() {
        let t = Timeouts::default();
        assert_eq!(t.timeout_duration(TimeoutKind::Propose), t.timeout_propose);
        assert_eq!(t.timeout_duration(TimeoutKind::Prevote), t.timeout_prevote);
        assert_eq!(
            t.timeout_duration(TimeoutKind::Precommit),
            t.timeout_precommit
        );
    }
}
