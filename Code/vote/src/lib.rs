//! Tally votes of the same type (eg. prevote or precommit)

#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    // missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]

extern crate alloc;

pub mod count;
pub mod keeper;
pub mod round_votes;
pub mod value_weights;

// TODO: Introduce newtype
// QUESTION: Over what type? i64?
pub type Weight = u64;

/// Represents the different quorum thresholds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Threshold<ValueId> {
    /// No quorum has been reached yet
    Unreached,
    /// Qorum of votes but not for the same value
    Any,
    /// Quorum for nil
    Nil,
    /// Quorum for a value
    Value(ValueId),
}
