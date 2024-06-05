//! Common data types and abstractions for the consensus engine.

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
mod block_part;
mod context;
mod height;
mod proposal;
mod round;
mod signed_block_part;
mod signed_proposal;
mod signed_vote;
mod signing;
mod timeout;

mod transaction;
mod validator_set;
mod value;
mod vote;

// Re-export `signature` crate for convenience
pub use ::signature;

/// Type alias to make it easier to refer the `ValueId` type of a given `Consensus` engine.
pub type ValueId<Ctx> = <<Ctx as Context>::Value as Value>::Id;

/// Type alias to make it easier to refer the `PublicKey` type of a given `Consensus` engine.
pub type PublicKey<Ctx> = <<Ctx as Context>::SigningScheme as SigningScheme>::PublicKey;

/// Type alias to make it easier to refer the `PrivateKey` type of a given `Consensus` engine.
pub type PrivateKey<Ctx> = <<Ctx as Context>::SigningScheme as SigningScheme>::PrivateKey;

/// Type alias to make it easier to refer the `Signature` type of a given `Consensus` engine.
pub type Signature<Ctx> = <<Ctx as Context>::SigningScheme as SigningScheme>::Signature;

pub use block_part::BlockPart;
pub use context::Context;
pub use height::Height;
pub use proposal::Proposal;
pub use round::Round;
pub use signed_block_part::SignedBlockPart;
pub use signed_proposal::SignedProposal;
pub use signed_vote::SignedVote;
pub use signing::SigningScheme;
pub use timeout::{Timeout, TimeoutStep};
pub use transaction::MempoolTransactionBatch;
pub use transaction::Transaction;
pub use transaction::TransactionBatch;
pub use validator_set::{Address, Validator, ValidatorSet, VotingPower};
pub use value::{NilOrVal, Value};
pub use vote::{Vote, VoteType};
