//! Infrastructure for tallying votes within the consensus engine.

#![no_std]
#![forbid(unsafe_code)]
#![deny(trivial_casts, trivial_numeric_casts)]
#![warn(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]

extern crate alloc;

pub mod count;
pub mod evidence;
pub mod keeper;
pub mod round_votes;
pub mod round_weights;
pub mod value_weights;

pub use evidence::EvidenceMap;

/// Represents the weight of a vote,
/// ie. the voting power of the validator that cast the vote.
pub type Weight = malachitebft_core_types::VotingPower;

pub use malachitebft_core_types::{Threshold, ThresholdParam, ThresholdParams};
