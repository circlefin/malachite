//! Byzantine behavior configuration and trigger types.
//!
//! [`ByzantineConfig`] is the top-level configuration for a Byzantine node,
//! specifying which attacks to perform and when they fire.
//!
//! [`Trigger`] specifies the timing of an attack: never (default), always,
//! randomly, at specific heights/rounds, or within a height range.

use eyre::{bail, Result};
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
/// ignore_locks = { mode = "always" }
/// seed = 42
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ByzantineConfig {
    /// When to send conflicting votes (equivocation).
    pub equivocate_votes: Trigger,

    /// When to send conflicting proposals (equivocation).
    pub equivocate_proposals: Trigger,

    /// When to drop outgoing votes (silence / censorship).
    pub drop_votes: Trigger,

    /// When to drop outgoing proposals (silence / censorship).
    pub drop_proposals: Trigger,

    /// When to ignore voting locks (amnesia attack).
    ///
    /// When triggered, middleware overrides nil **prevotes** with the most
    /// recently observed proposal value for the same `(height, round)`.
    pub ignore_locks: Trigger,

    /// Random seed for reproducible random attacks.
    ///
    /// If set, the random number generator is seeded with this value,
    /// making random triggers reproducible given the same trigger-evaluation
    /// order.
    pub seed: Option<u64>,
}

impl ByzantineConfig {
    pub fn new(seed: Option<u64>) -> Self {
        Self {
            seed,
            ..Default::default()
        }
    }

    pub fn with_equivocate_votes(mut self, trigger: Trigger) -> Self {
        self.equivocate_votes = trigger;
        self
    }

    pub fn with_equivocate_proposals(mut self, trigger: Trigger) -> Self {
        self.equivocate_proposals = trigger;
        self
    }

    pub fn with_drop_votes(mut self, trigger: Trigger) -> Self {
        self.drop_votes = trigger;
        self
    }

    pub fn with_drop_proposals(mut self, trigger: Trigger) -> Self {
        self.drop_proposals = trigger;
        self
    }

    pub fn with_ignore_locks(mut self, trigger: Trigger) -> Self {
        self.ignore_locks = trigger;
        self
    }

    /// Returns `true` if any Byzantine behavior is configured.
    pub fn is_active(&self) -> bool {
        self.equivocate_votes.is_set()
            || self.equivocate_proposals.is_set()
            || self.drop_votes.is_set()
            || self.drop_proposals.is_set()
            || self.ignore_locks.is_set()
    }

    /// Validate trigger parameters and reject invalid configuration.
    pub fn validate(&self) -> Result<()> {
        self.equivocate_votes.validate("equivocate_votes")?;
        self.equivocate_proposals.validate("equivocate_proposals")?;
        self.drop_votes.validate("drop_votes")?;
        self.drop_proposals.validate("drop_proposals")?;
        self.ignore_locks.validate("ignore_locks")?;

        if self.drop_votes.is_set() && self.equivocate_votes.is_set() {
            bail!("drop_votes and equivocate_votes cannot both be set");
        }
        if self.drop_proposals.is_set() && self.equivocate_proposals.is_set() {
            bail!("drop_proposals and equivocate_proposals cannot both be set");
        }

        Ok(())
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum Trigger {
    /// Never fire. This is the default for unconfigured attacks.
    #[default]
    #[serde(rename = "never")]
    Never,

    /// Always fire when this trigger is evaluated.
    #[serde(rename = "always")]
    Always,

    /// Fire randomly with a given probability (must be in `[0.0, 1.0]`).
    #[serde(rename = "random")]
    Random {
        /// Probability of firing. Valid range: `0.0` (never) to `1.0` (always).
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

    /// Fire within a height range `[from, to]` (inclusive, with `from <= to`).
    #[serde(rename = "height_range")]
    HeightRange {
        /// Start of the height range (inclusive).
        from: u64,
        /// End of the height range (inclusive).
        to: u64,
    },
}

impl Trigger {
    /// Returns `true` if this trigger is configured (not `Never`).
    pub fn is_set(&self) -> bool {
        *self != Trigger::Never
    }

    /// Validate trigger-specific invariants.
    pub fn validate(&self, field_name: &str) -> Result<()> {
        match self {
            Trigger::Random { probability } => {
                if !probability.is_finite() || !(0.0..=1.0).contains(probability) {
                    bail!("invalid {field_name}.probability: {probability} (expected finite value in [0.0, 1.0])");
                }
            }
            Trigger::HeightRange { from, to } => {
                if from > to {
                    bail!("invalid {field_name}.height_range: from ({from}) must be <= to ({to})");
                }
                if *from == 0 || *to == 0 {
                    bail!("invalid {field_name}.height_range: from ({from}) and to ({to}) must be > 0");
                }
            }
            Trigger::AtHeights { heights } => {
                if heights.is_empty() {
                    bail!("invalid {field_name}.heights: list must not be empty");
                }
                if heights.contains(&0) {
                    bail!("invalid {field_name}.heights: heights must be > 0");
                }
            }
            Trigger::AtRounds { rounds } => {
                if rounds.is_empty() {
                    bail!("invalid {field_name}.rounds: list must not be empty");
                }
                if rounds.iter().any(|r| *r < 0) {
                    bail!("invalid {field_name}.rounds: rounds must be >= 0");
                }
            }
            Trigger::Never | Trigger::Always => {}
        }

        Ok(())
    }

    /// Evaluate whether this trigger fires for the given height and round.
    pub fn fires<H: Height>(&self, height: H, round: Round, rng: &mut StdRng) -> bool {
        let h = height.as_u64();
        let r = round.as_i64();

        match self {
            Trigger::Never => false,
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
            equivocate_votes: Trigger::Random { probability: 0.3 },
            drop_votes: Trigger::AtHeights {
                heights: vec![10, 20, 30],
            },
            drop_proposals: Trigger::HeightRange { from: 50, to: 100 },
            ignore_locks: Trigger::Always,
            seed: Some(42),
            ..Default::default()
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: ByzantineConfig = toml::from_str(&toml_str).unwrap();

        assert!(parsed.equivocate_votes.is_set());
        assert!(parsed.drop_votes.is_set());
        assert!(parsed.drop_proposals.is_set());
        assert!(parsed.ignore_locks.is_set());
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

    #[test]
    fn test_validate_rejects_invalid_random_probability() {
        let config = ByzantineConfig {
            equivocate_votes: Trigger::Random { probability: 1.5 },
            ..Default::default()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("probability"));
    }

    #[test]
    fn test_validate_rejects_invalid_height_range() {
        let config = ByzantineConfig {
            drop_votes: Trigger::HeightRange { from: 10, to: 5 },
            ..Default::default()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("height_range"));
    }

    #[test]
    fn test_validate_rejects_zero_height_range_bounds() {
        let config = ByzantineConfig {
            drop_votes: Trigger::HeightRange { from: 0, to: 5 },
            ..Default::default()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("height_range"));
    }

    #[test]
    fn test_validate_rejects_empty_heights() {
        let config = ByzantineConfig {
            drop_votes: Trigger::AtHeights { heights: vec![] },
            ..Default::default()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("heights"));
    }

    #[test]
    fn test_validate_rejects_zero_heights() {
        let config = ByzantineConfig {
            drop_votes: Trigger::AtHeights {
                heights: vec![0, 10],
            },
            ..Default::default()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("heights"));
    }

    #[test]
    fn test_validate_rejects_empty_rounds() {
        let config = ByzantineConfig {
            drop_votes: Trigger::AtRounds { rounds: vec![] },
            ..Default::default()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("rounds"));
    }

    #[test]
    fn test_validate_rejects_negative_rounds() {
        let config = ByzantineConfig {
            drop_votes: Trigger::AtRounds {
                rounds: vec![-1, 2],
            },
            ..Default::default()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("rounds"));
    }

    #[test]
    fn test_validate_rejects_drop_and_equivocate_votes() {
        let config = ByzantineConfig {
            drop_votes: Trigger::Always,
            equivocate_votes: Trigger::Always,
            ..Default::default()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("drop_votes and equivocate_votes"));
    }

    #[test]
    fn test_validate_rejects_drop_and_equivocate_proposals() {
        let config = ByzantineConfig {
            drop_proposals: Trigger::Random { probability: 0.5 },
            equivocate_proposals: Trigger::Always,
            ..Default::default()
        };

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("drop_proposals and equivocate_proposals"));
    }
}
