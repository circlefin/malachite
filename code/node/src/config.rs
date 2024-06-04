use std::time::Duration;

use malachite_common::TimeoutStep;
use multiaddr::Multiaddr;
use serde::{Deserialize, Serialize};

/// Malachite configuration options
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// A custom human-readable name for this node
    pub moniker: String,
    /// Consensus configuration options
    pub consensus: ConsensusConfig,
    /// Mempool configuration options
    pub mempool: MempoolConfig,
}

/// P2P configuration options
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct P2pConfig {
    // Address to listen for incoming connections
    pub listen_addr: Multiaddr,
    /// List of nodes to keep persistent connections to
    pub persistent_peers: Vec<Multiaddr>,
}

/// Mempool configuration options
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MempoolConfig {
    /// P2P configuration options
    pub p2p: P2pConfig,
}

/// Consensus configuration options
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Timeouts
    #[serde(flatten)]
    pub timeouts: TimeoutConfig,

    /// P2P configuration options
    pub p2p: P2pConfig,
}

/// Timeouts
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// How long we wait for a proposal block before prevoting nil
    #[serde(with = "humantime_serde")]
    pub timeout_propose: Duration,

    /// How much timeout_propose increases with each round
    #[serde(with = "humantime_serde")]
    pub timeout_propose_delta: Duration,

    /// How long we wait after receiving +2/3 prevotes for “anything” (ie. not a single block or nil)
    #[serde(with = "humantime_serde")]
    pub timeout_prevote: Duration,

    /// How much the timeout_prevote increases with each round
    #[serde(with = "humantime_serde")]
    pub timeout_prevote_delta: Duration,

    /// How long we wait after receiving +2/3 precommits for “anything” (ie. not a single block or nil)
    #[serde(with = "humantime_serde")]
    pub timeout_precommit: Duration,

    /// How much the timeout_precommit increases with each round
    #[serde(with = "humantime_serde")]
    pub timeout_precommit_delta: Duration,

    /// How long we wait after committing a block, before starting on the new
    /// height (this gives us a chance to receive some more precommits, even
    /// though we already have +2/3).
    #[serde(with = "humantime_serde")]
    pub timeout_commit: Duration,
}

impl TimeoutConfig {
    pub fn timeout_duration(&self, step: TimeoutStep) -> Duration {
        match step {
            TimeoutStep::Propose => self.timeout_propose,
            TimeoutStep::Prevote => self.timeout_prevote,
            TimeoutStep::Precommit => self.timeout_precommit,
            TimeoutStep::Commit => self.timeout_commit,
        }
    }

    pub fn delta_duration(&self, step: TimeoutStep) -> Option<Duration> {
        match step {
            TimeoutStep::Propose => Some(self.timeout_propose_delta),
            TimeoutStep::Prevote => Some(self.timeout_prevote_delta),
            TimeoutStep::Precommit => Some(self.timeout_precommit_delta),
            TimeoutStep::Commit => None,
        }
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            timeout_propose: Duration::from_secs(3),
            timeout_propose_delta: Duration::from_millis(500),
            timeout_prevote: Duration::from_secs(1),
            timeout_prevote_delta: Duration::from_millis(500),
            timeout_precommit: Duration::from_secs(1),
            timeout_precommit_delta: Duration::from_millis(500),
            timeout_commit: Duration::from_secs(1),
        }
    }
}
