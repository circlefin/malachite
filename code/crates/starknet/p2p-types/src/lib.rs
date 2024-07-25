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

mod signing;

pub type SigningScheme = signing::Ecdsa;
pub type Signature = signing::Signature;
pub type PublicKey = signing::PublicKey;
pub type PrivateKey = signing::PrivateKey;
