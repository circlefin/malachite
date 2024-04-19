use core::fmt;

use malachite_common::{NilOrVal, Round, VoteType};
use subtle_encoding::hex;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Hash([u8; 32]);

impl Hash {
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }
}

impl fmt::Display for Hash {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::str::from_utf8(&hex::encode(self.0)).unwrap().fmt(f)
    }
}

impl core::str::FromStr for Hash {
    type Err = Box<dyn std::error::Error>;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(hex::decode(s)?.as_slice().try_into()?))
    }
}

pub type Height = malachite_test::Height;
pub type Validator = malachite_test::Validator;
pub type Address = malachite_test::Address;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BlockHash(Hash);

impl BlockHash {
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(Hash::new(hash))
    }
}

impl fmt::Display for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockHash({})", self.0)
    }
}

impl core::str::FromStr for BlockHash {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(hex::decode(s)?.as_slice().try_into()?))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MessageHash(Hash);

impl MessageHash {
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(Hash::new(hash))
    }
}

impl fmt::Display for MessageHash {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MessageHash({})", self.0)
    }
}

impl core::str::FromStr for MessageHash {
    type Err = Box<dyn std::error::Error>;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(hex::decode(s)?.as_slice().try_into()?))
    }
}

pub type Signature = malachite_test::Signature;
pub type PublicKey = malachite_test::PublicKey;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote {
    pub typ: VoteType,
    pub height: Height,
    pub round: Round,
    pub value: NilOrVal<BlockHash>,
    pub validator_address: Address,
}

pub type Precommit = Vote;

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub height: Height,
    pub round: Round,
    pub value: BlockHash,
    pub pol_round: Round,
    pub validator_address: Address,
}

#[derive(Clone, Debug)]
pub enum ProposalContent {
    Tx(TxContent),
    Proof(ProofContent),
}

#[derive(Clone, Debug)]
pub struct TxContent {
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ProofContent {
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum Message {
    Proposal(Proposal),
    Vote(Vote),
}
