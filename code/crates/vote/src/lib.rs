//! Infrastructure for tallying votes within the consensus engine.

#![no_std]
#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

pub mod count;
pub mod evidence;
pub mod keeper;
pub mod round_votes;
pub mod round_weights;
pub mod value_weights;

/// Represents the weight of a vote,
/// ie. the voting power of the validator that cast the vote.
pub type Weight = malachite_common::VotingPower;

pub use malachite_common::{Threshold, ThresholdParam, ThresholdParams};
