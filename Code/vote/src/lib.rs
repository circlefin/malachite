//! Infrastructre for tallying votes within the consensus engine.

#![no_std]
#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use malachite_common::VotingPower;

extern crate alloc;

pub mod count;
pub mod keeper;
pub mod round_votes;
pub mod round_weights;
pub mod value_weights;

// TODO: Introduce newtype
/// Represents the weight of a vote,
/// ie. the voting power of the validator that cast the vote.
pub type Weight = VotingPower;

/// Represents the different quorum thresholds.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Threshold<ValueId> {
    /// No quorum has been reached yet
    Unreached,

    /// Quorum of votes but not for the same value
    Any,

    /// Quorum of votes for nil
    Nil,

    /// Quorum (+2/3) of votes for a value
    Value(ValueId),
}

/// Represents the different quorum thresholds.
///
/// There are two thresholds:
/// - The quorum threshold, which is the minimum number of votes required for a quorum.
/// - The honest threshold, which is the minimum number of votes required for a quorum of honest nodes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ThresholdParams {
    /// Threshold for a quorum (default: 2f+1)
    pub quorum: ThresholdParam,

    /// Threshold for the minimum number of honest nodes (default: f+1)
    pub honest: ThresholdParam,
}

impl Default for ThresholdParams {
    fn default() -> Self {
        Self {
            quorum: ThresholdParam::TWO_F_PLUS_ONE,
            honest: ThresholdParam::F_PLUS_ONE,
        }
    }
}

// TODO: Distinguish between quorum and honest thresholds at the type-level
/// Represents the different quorum thresholds.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ThresholdParam {
    /// Numerator of the threshold
    pub numerator: u64,
    /// Denominator of the threshold
    pub denominator: u64,
}

impl ThresholdParam {
    /// 2f+1, ie. more than two thirds of the total weight
    pub const TWO_F_PLUS_ONE: Self = Self::new(2, 3);

    /// f+1, ie. more than one third of the total weight
    pub const F_PLUS_ONE: Self = Self::new(1, 3);

    /// Create a new threshold parameter with the given numerator and denominator.
    pub const fn new(numerator: u64, denominator: u64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// Check whether the threshold is met.
    pub const fn is_met(&self, weight: Weight, total: Weight) -> bool {
        // FIXME: Deal with overflows
        weight * self.denominator > total * self.numerator
    }
}
