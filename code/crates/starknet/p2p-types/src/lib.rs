// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod address;
pub use address::Address;

mod height;
pub use height::Height;

mod vote;
pub use vote::Vote;

mod proposal;
pub use proposal::Proposal;

mod transaction;
pub use transaction::{Transaction, Transactions};

mod validator;
pub use validator::Validator;

mod validator_set;
pub use validator_set::ValidatorSet;

mod proposal_part;
pub use proposal_part::{ProposalFin, ProposalInit, ProposalMessage, ProposalPart};

mod block_proof;
pub use block_proof::BlockProof;

mod hash;
pub use hash::{BlockHash, Hash, MessageHash};

mod streaming;
pub use streaming::{Stream, StreamContent};

mod crypto;

pub type SigningScheme = crypto::Ed25519;
pub type Signature = crypto::Signature;
pub type PublicKey = crypto::PublicKey;
pub type PrivateKey = crypto::PrivateKey;
