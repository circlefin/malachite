//! Byzantine behavior configuration and trigger types.
//!
//! [`ByzantineConfig`] is the top-level configuration for a Byzantine node,
//! specifying which attacks to perform and when they fire.
//!
//! [`Trigger`] specifies the timing of an attack: always, randomly, at
//! specific heights/rounds, or within a height range.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use malachitebft_core_types::{Height, Round};

/// Top-level Byzantine behavior configuration.
///
/// This struct is TOML-serializable and can be embedded in the node's
/// `config.toml` under a `[byzantine]` section.
///
/// # Example
///
/// ```toml
/// [byzantine]
/// equivocate_votes = { mode = "random", probability = 0.3 }
/// drop_proposals = { mode = "at_heights", heights = [10, 20, 30] }
/// ignore_locks = true
/// seed = 42
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ByzantineConfig {
    /// When to send conflicting votes (equivocation).
    #[serde(default)]
    pub equivocate_votes: Option<Trigger>,

    /// When to send conflicting proposals (equivocation).
    #[serde(default)]
    pub equivocate_proposals: Option<Trigger>,

    /// When to drop outgoing votes (silence / censorship).
    #[serde(default)]
    pub drop_votes: Option<Trigger>,

    /// When to drop outgoing proposals (silence / censorship).
    #[serde(default)]
    pub drop_proposals: Option<Trigger>,

    /// Whether to ignore voting locks (amnesia attack).
    ///
    /// When `true`, the node will vote for the proposed value even when
    /// locked on a different value.
    #[serde(default)]
    pub ignore_locks: bool,

    /// Random seed for reproducible random attacks.
    ///
    /// If set, the random number generator is seeded with this value,
    /// making random triggers deterministic across runs.
    #[serde(default)]
    pub seed: Option<u64>,
}

impl ByzantineConfig {
    /// Returns `true` if any Byzantine behavior is configured.
    pub fn is_active(&self) -> bool {
        self.equivocate_votes.is_some()
            || self.equivocate_proposals.is_some()
            || self.drop_votes.is_some()
            || self.drop_proposals.is_some()
            || self.ignore_locks
    }
}

/// Specifies **when** a Byzantine attack fires.
///
/// Triggers support both controlled (deterministic) and random modes,
/// and are fully TOML-serializable via the `mode` tag.
///
/// # TOML examples
///
/// ```toml
/// # Always fire
/// trigger = { mode = "always" }
///
/// # Fire randomly 20% of the time
/// trigger = { mode = "random", probability = 0.2 }
///
/// # Fire at specific heights
/// trigger = { mode = "at_heights", heights = [10, 20, 30] }
///
/// # Fire at specific rounds (within any height)
/// trigger = { mode = "at_rounds", rounds = [2, 3] }
///
/// # Fire within a height range (inclusive)
/// trigger = { mode = "height_range", from = 50, to = 100 }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum Trigger {
    /// Fire on every message.
    #[serde(rename = "always")]
    Always,

    /// Fire randomly with a given probability (0.0 to 1.0).
    #[serde(rename = "random")]
    Random {
        /// Probability of firing, between 0.0 (never) and 1.0 (always).
        probability: f64,
    },

    /// Fire at specific heights.
    #[serde(rename = "at_heights")]
    AtHeights {
        /// The set of heights at which the attack fires.
        heights: Vec<u64>,
    },

    /// Fire at specific rounds (within any height).
    #[serde(rename = "at_rounds")]
    AtRounds {
        /// The set of rounds at which the attack fires.
        rounds: Vec<i64>,
    },

    /// Fire within a height range `[from, to]` (inclusive).
    #[serde(rename = "height_range")]
    HeightRange {
        /// Start of the height range (inclusive).
        from: u64,
        /// End of the height range (inclusive).
        to: u64,
    },
}

impl Trigger {
    /// Evaluate whether this trigger fires for the given height and round.
    pub fn fires<H: Height>(&self, height: H, round: Round, rng: &mut StdRng) -> bool {
        let h = height.as_u64();
        let r = round.as_i64();

        match self {
            Trigger::Always => true,
            Trigger::Random { probability } => rng.gen::<f64>() < *probability,
            Trigger::AtHeights { heights } => heights.contains(&h),
            Trigger::AtRounds { rounds } => rounds.contains(&r),
            Trigger::HeightRange { from, to } => h >= *from && h <= *to,
        }
    }
}

/// Creates a [`StdRng`] from an optional seed, or from entropy if `None`.
pub fn make_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toml_roundtrip() {
        let config = ByzantineConfig {
            equivocate_votes: Some(Trigger::Random { probability: 0.3 }),
            equivocate_proposals: None,
            drop_votes: Some(Trigger::AtHeights {
                heights: vec![10, 20, 30],
            }),
            drop_proposals: Some(Trigger::HeightRange { from: 50, to: 100 }),
            ignore_locks: true,
            seed: Some(42),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: ByzantineConfig = toml::from_str(&toml_str).unwrap();

        assert!(parsed.equivocate_votes.is_some());
        assert!(parsed.drop_votes.is_some());
        assert!(parsed.drop_proposals.is_some());
        assert!(parsed.ignore_locks);
        assert_eq!(parsed.seed, Some(42));
    }

    #[test]
    fn test_empty_config_is_inactive() {
        let config = ByzantineConfig::default();
        assert!(!config.is_active());
    }

    #[test]
    fn test_trigger_always() {
        let trigger = Trigger::Always;
        let mut rng = make_rng(Some(0));
        assert!(trigger.fires(malachitebft_test::Height::new(1), Round::new(0), &mut rng));
    }

    #[test]
    fn test_trigger_at_heights() {
        let trigger = Trigger::AtHeights {
            heights: vec![5, 10],
        };
        let mut rng = make_rng(Some(0));
        assert!(!trigger.fires(malachitebft_test::Height::new(1), Round::new(0), &mut rng));
        assert!(trigger.fires(malachitebft_test::Height::new(5), Round::new(0), &mut rng));
        assert!(trigger.fires(malachitebft_test::Height::new(10), Round::new(0), &mut rng));
    }

    #[test]
    fn test_trigger_height_range() {
        let trigger = Trigger::HeightRange { from: 5, to: 10 };
        let mut rng = make_rng(Some(0));
        assert!(!trigger.fires(malachitebft_test::Height::new(4), Round::new(0), &mut rng));
        assert!(trigger.fires(malachitebft_test::Height::new(5), Round::new(0), &mut rng));
        assert!(trigger.fires(malachitebft_test::Height::new(7), Round::new(0), &mut rng));
        assert!(trigger.fires(malachitebft_test::Height::new(10), Round::new(0), &mut rng));
        assert!(!trigger.fires(malachitebft_test::Height::new(11), Round::new(0), &mut rng));
    }
}
