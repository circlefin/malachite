use crate::hash;
use crate::mock::context::MockContext;

mod block_part;
pub use block_part::{BlockMetadata, BlockPart};

mod content;
pub use content::ProposalContent;

mod vote;
pub use vote::Vote;

mod proposal;
pub use proposal::Proposal;

mod transaction;
pub use transaction::{Transaction, TransactionBatch};

mod validator;
pub use validator::Validator;

mod validator_set;
pub use validator_set::ValidatorSet;

mod proposal_part;
pub use proposal_part::ProposalPart;

pub type StarknetContext = MockContext;

pub type Height = malachite_test::Height;
pub type Address = malachite_test::Address;
pub type SigningScheme = malachite_test::Ed25519;

pub type Hash = hash::Hash;
pub type MessageHash = hash::MessageHash;
pub type BlockHash = hash::BlockHash;

pub type Signature = malachite_test::Signature;
pub type PublicKey = malachite_test::PublicKey;
pub type PrivateKey = malachite_test::PrivateKey;
