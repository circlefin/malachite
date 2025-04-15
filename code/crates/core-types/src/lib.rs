//! Common data types and abstractions for the consensus engine.

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

mod certificate;
mod context;
mod height;
mod proposal;
mod proposal_part;
mod round;
mod signed_message;
mod signing;
mod threshold;
mod timeout;
mod validator_set;
mod value;
mod vote;
mod vote_extension;
mod vote_set;

/// Type alias to make it easier to refer the `ValueId` type.
pub type ValueId<Ctx> = <<Ctx as Context>::Value as Value>::Id;

/// Type alias to make it easier to refer the `PublicKey` type.
pub type PublicKey<Ctx> = <<Ctx as Context>::SigningScheme as SigningScheme>::PublicKey;

/// Type alias to make it easier to refer the `PrivateKey` type.
pub type PrivateKey<Ctx> = <<Ctx as Context>::SigningScheme as SigningScheme>::PrivateKey;

/// Type alias to make it easier to refer the `Signature` type.
pub type Signature<Ctx> = <<Ctx as Context>::SigningScheme as SigningScheme>::Signature;

/// A signed vote
pub type SignedVote<Ctx> = SignedMessage<Ctx, <Ctx as Context>::Vote>;

/// A signed proposal
pub type SignedProposal<Ctx> = SignedMessage<Ctx, <Ctx as Context>::Proposal>;

/// A signed proposal part
pub type SignedProposalPart<Ctx> = SignedMessage<Ctx, <Ctx as Context>::ProposalPart>;

/// A signed vote extension
pub type SignedExtension<Ctx> = SignedMessage<Ctx, <Ctx as Context>::Extension>;

pub use certificate::{
    CertificateError, CommitCertificate, CommitSignature, PolkaCertificate, PolkaSignature,
};
pub use context::Context;
pub use height::Height;
pub use proposal::{Proposal, Validity};
pub use proposal_part::ProposalPart;
pub use round::Round;
pub use signed_message::SignedMessage;
pub use signing::{SigningProvider, SigningProviderExt, SigningScheme};
pub use threshold::{Threshold, ThresholdParam, ThresholdParams};
pub use timeout::{Timeout, TimeoutKind};
pub use validator_set::{Address, Validator, ValidatorSet, VotingPower};
pub use value::{NilOrVal, Value, ValueOrigin, ValuePayload};
pub use vote::{Vote, VoteType};
pub use vote_extension::{Extension, VoteExtensions};
pub use vote_set::VoteSet;
