use core::fmt::Debug;
use core::time::Duration;

use crate::{Context, Round, Timeout, TimeoutKind};

/// Timeouts control how long the consensus engine waits for various steps
/// in the consensus protocol.
///
/// The standard implementation is [`LinearTimeouts`], which should be used
/// unless you have specific requirements for custom timeout behavior. See
/// [`LinearTimeouts::default`] for the default values.
pub trait Timeouts<Ctx>
where
    Self: Clone + Debug + Eq + Send + Sync + Copy,
    Ctx: Context,
{
    /// Get the duration for a given step and round.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout to get the duration for.
    ///
    /// # Returns
    ///
    /// The duration for the given timeout
    ///
    /// # Panics
    ///
    /// If the timeout round is nil, this function must panic.
    fn duration_for(&self, timeout: Timeout) -> Duration;
}

/// Timeouts that increase linearly with the round number.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LinearTimeouts {
    /// How long we wait for a proposal block before prevoting nil
    pub propose: Duration,

    /// How much timeout_propose increases with each round
    pub propose_delta: Duration,

    /// How long we wait after receiving +2/3 prevotes for “anything” (ie. not a single block or nil)
    pub prevote: Duration,

    /// How much the timeout_prevote increases with each round
    pub prevote_delta: Duration,

    /// How long we wait after receiving +2/3 precommits for “anything” (ie. not a single block or nil)
    pub precommit: Duration,

    /// How much the timeout_precommit increases with each round
    pub precommit_delta: Duration,

    /// How long we wait after entering a round before starting
    /// the rebroadcast liveness protocol
    pub rebroadcast: Duration,
}

impl<Ctx: Context> Timeouts<Ctx> for LinearTimeouts {
    fn duration_for(&self, timeout: Timeout) -> Duration {
        self.duration_for(timeout.kind, timeout.round)
    }
}

impl Default for LinearTimeouts {
    fn default() -> Self {
        let propose = Duration::from_secs(3);
        let prevote = Duration::from_secs(1);
        let precommit = Duration::from_secs(1);
        let rebroadcast = propose + prevote + precommit;
        Self {
            propose,
            propose_delta: Duration::from_millis(500),
            prevote,
            prevote_delta: Duration::from_millis(500),
            precommit,
            precommit_delta: Duration::from_millis(500),
            rebroadcast,
        }
    }
}

impl LinearTimeouts {
    /// See [`Timeouts::duration_for`].
    pub fn duration_for(&self, kind: TimeoutKind, round: Round) -> Duration {
        let round = round.as_u32().expect("Round must be defined");

        match kind {
            TimeoutKind::Propose => self.propose + self.propose_delta * round,
            TimeoutKind::Prevote => self.prevote + self.prevote_delta * round,
            TimeoutKind::Precommit => self.precommit + self.precommit_delta * round,
            TimeoutKind::Rebroadcast => {
                self.rebroadcast
                    + (self.propose_delta + self.prevote_delta + self.precommit_delta) * round
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Round;

    #[test]
    fn test_default_timeouts() {
        let timeouts = LinearTimeouts::default();

        assert_eq!(timeouts.propose, Duration::from_secs(3));
        assert_eq!(timeouts.propose_delta, Duration::from_millis(500));
        assert_eq!(timeouts.prevote, Duration::from_secs(1));
        assert_eq!(timeouts.prevote_delta, Duration::from_millis(500));
        assert_eq!(timeouts.precommit, Duration::from_secs(1));
        assert_eq!(timeouts.precommit_delta, Duration::from_millis(500));
        assert_eq!(timeouts.rebroadcast, Duration::from_secs(5)); // 3 + 1 + 1
    }

    #[test]
    fn test_propose_timeout_increases_linearly() {
        let timeouts = LinearTimeouts::default();

        // Round 0: 3s
        let r0 = timeouts.duration_for(TimeoutKind::Propose, Round::new(0));
        assert_eq!(r0, Duration::from_secs(3));

        // Round 1: 3s + 0.5s = 3.5s
        let r1 = timeouts.duration_for(TimeoutKind::Propose, Round::new(1));
        assert_eq!(r1, Duration::from_millis(3500));

        // Round 2: 3s + 1s = 4s
        let r2 = timeouts.duration_for(TimeoutKind::Propose, Round::new(2));
        assert_eq!(r2, Duration::from_secs(4));

        // Round 10: 3s + 5s = 8s
        let r10 = timeouts.duration_for(TimeoutKind::Propose, Round::new(10));
        assert_eq!(r10, Duration::from_secs(8));
    }

    #[test]
    fn test_prevote_timeout_increases_linearly() {
        let timeouts = LinearTimeouts::default();

        // Round 0: 1s
        let r0 = timeouts.duration_for(TimeoutKind::Prevote, Round::new(0));
        assert_eq!(r0, Duration::from_secs(1));

        // Round 1: 1s + 0.5s = 1.5s
        let r1 = timeouts.duration_for(TimeoutKind::Prevote, Round::new(1));
        assert_eq!(r1, Duration::from_millis(1500));

        // Round 2: 1s + 1s = 2s
        let r2 = timeouts.duration_for(TimeoutKind::Prevote, Round::new(2));
        assert_eq!(r2, Duration::from_secs(2));

        // Round 10: 1s + 5s = 6s
        let r10 = timeouts.duration_for(TimeoutKind::Prevote, Round::new(10));
        assert_eq!(r10, Duration::from_secs(6));
    }

    #[test]
    fn test_precommit_timeout_increases_linearly() {
        let timeouts = LinearTimeouts::default();

        // Round 0: 1s
        let r0 = timeouts.duration_for(TimeoutKind::Precommit, Round::new(0));
        assert_eq!(r0, Duration::from_secs(1));

        // Round 1: 1s + 0.5s = 1.5s
        let r1 = timeouts.duration_for(TimeoutKind::Precommit, Round::new(1));
        assert_eq!(r1, Duration::from_millis(1500));

        // Round 2: 1s + 1s = 2s
        let r2 = timeouts.duration_for(TimeoutKind::Precommit, Round::new(2));
        assert_eq!(r2, Duration::from_secs(2));

        // Round 10: 1s + 5s = 6s
        let r10 = timeouts.duration_for(TimeoutKind::Precommit, Round::new(10));
        assert_eq!(r10, Duration::from_secs(6));
    }

    #[test]
    fn test_rebroadcast_timeout_increases_linearly() {
        let timeouts = LinearTimeouts::default();

        // Round 0: 5s
        let r0 = timeouts.duration_for(TimeoutKind::Rebroadcast, Round::new(0));
        assert_eq!(r0, Duration::from_secs(5));

        // Round 1: 5s + (0.5s + 0.5s + 0.5s) = 5s + 1.5s = 6.5s
        let r1 = timeouts.duration_for(TimeoutKind::Rebroadcast, Round::new(1));
        assert_eq!(r1, Duration::from_millis(6500));

        // Round 2: 5s + 3s = 8s
        let r2 = timeouts.duration_for(TimeoutKind::Rebroadcast, Round::new(2));
        assert_eq!(r2, Duration::from_secs(8));

        // Round 10: 5s + 15s = 20s
        let r10 = timeouts.duration_for(TimeoutKind::Rebroadcast, Round::new(10));
        assert_eq!(r10, Duration::from_secs(20));
    }

    #[test]
    fn test_custom_timeouts() {
        let timeouts = LinearTimeouts {
            propose: Duration::from_secs(5),
            propose_delta: Duration::from_secs(1),
            prevote: Duration::from_secs(2),
            prevote_delta: Duration::from_millis(100),
            precommit: Duration::from_secs(3),
            precommit_delta: Duration::from_millis(200),
            rebroadcast: Duration::from_secs(10),
        };

        // Test propose at round 3: 5s + 3*1s = 8s
        let propose_r3 = timeouts.duration_for(TimeoutKind::Propose, Round::new(3));
        assert_eq!(propose_r3, Duration::from_secs(8));

        // Test prevote at round 5: 2s + 5*0.1s = 2.5s
        let prevote_r5 = timeouts.duration_for(TimeoutKind::Prevote, Round::new(5));
        assert_eq!(prevote_r5, Duration::from_millis(2500));

        // Test precommit at round 4: 3s + 4*0.2s = 3.8s
        let precommit_r4 = timeouts.duration_for(TimeoutKind::Precommit, Round::new(4));
        assert_eq!(precommit_r4, Duration::from_millis(3800));

        // Test rebroadcast at round 2: 10s + 2*(1s + 0.1s + 0.2s) = 10s + 2.6s = 12.6s
        let rebroadcast_r2 = timeouts.duration_for(TimeoutKind::Rebroadcast, Round::new(2));
        assert_eq!(rebroadcast_r2, Duration::from_millis(12600));
    }
}
