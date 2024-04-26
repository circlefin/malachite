use core::fmt;

use proto::prost::Message as _;
use proto::types::Any;
use sha2::{Digest, Sha256};
use subtle_encoding::hex;

use malachite_common::{NilOrVal, Round, VoteType};
use malachite_proto as proto;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Hash([u8; 32]);

impl Hash {
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl proto::Protobuf for Hash {
    type Proto = proto::ValueId;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self::new(
            proto
                .value
                .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?
                .try_into()
                .map_err(|_| proto::Error::Other("Invalid hash length".to_string()))?,
        ))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::ValueId {
            value: Some(self.0.to_vec()),
        })
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

    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
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

impl proto::Protobuf for BlockHash {
    type Proto = proto::ValueId;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self(Hash::from_proto(proto)?))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        self.0.to_proto()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MessageHash(Hash);

impl MessageHash {
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(Hash::new(hash))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
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

impl proto::Protobuf for MessageHash {
    type Proto = proto::ValueId;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self(Hash::from_proto(proto)?))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        self.0.to_proto()
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

impl Vote {
    pub fn to_bytes(&self) -> Vec<u8> {
        malachite_proto::Protobuf::to_bytes(self).unwrap()
    }
}

impl proto::Protobuf for Vote {
    type Proto = proto::Vote;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self {
            typ: VoteType::from(proto.vote_type()),
            height: Height::from_proto(
                proto
                    .height
                    .ok_or_else(|| proto::Error::missing_field::<proto::Vote>("height"))?,
            )?,
            round: Round::from_proto(
                proto
                    .round
                    .ok_or_else(|| proto::Error::missing_field::<proto::Vote>("round"))?,
            )?,
            value: match proto.value {
                Some(value) => NilOrVal::Val(BlockHash::from_proto(value)?),
                None => NilOrVal::Nil,
            },
            validator_address: Address::from_proto(
                proto.validator_address.ok_or_else(|| {
                    proto::Error::missing_field::<proto::Vote>("validator_address")
                })?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::Vote {
            vote_type: proto::VoteType::from(self.typ).into(),
            height: Some(self.height.to_proto()?),
            round: Some(self.round.to_proto()?),
            value: match &self.value {
                NilOrVal::Nil => None,
                NilOrVal::Val(v) => Some(v.to_proto()?),
            },
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }
}

pub type Precommit = Vote;

/// A block
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub txs: Vec<Tx>,
    pub proof: Proof,
}

impl proto::Protobuf for Block {
    type Proto = malachite_proto::Block;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self {
            txs: proto
                .txs
                .into_iter()
                .map(Tx::from_proto)
                .collect::<Result<_, _>>()?,
            proof: Proof::from_proto(
                proto
                    .proof
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("proof"))?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(malachite_proto::Block {
            txs: self
                .txs
                .iter()
                .map(Tx::to_proto)
                .collect::<Result<_, _>>()?,
            proof: Some(self.proof.to_proto()?),
        })
    }
}

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub height: Height,
    pub round: Round,
    pub value: Block,
    pub pol_round: Round,
    pub validator_address: Address,
}

impl Proposal {
    pub fn to_bytes(&self) -> Vec<u8> {
        proto::Protobuf::to_bytes(self).unwrap()
    }
}

impl proto::Protobuf for Proposal {
    type Proto = malachite_proto::Proposal;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        let value = proto::Value {
            value: Some(self.value.to_any()?.encode_to_vec()),
        };

        Ok(proto::Proposal {
            height: Some(self.height.to_proto()?),
            round: Some(self.round.to_proto()?),
            value: Some(value),
            pol_round: Some(self.pol_round.to_proto()?),
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let value = proto
            .value
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?;

        Ok(Self {
            height: Height::from_proto(
                proto
                    .height
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("height"))?,
            )?,
            round: Round::from_proto(
                proto
                    .round
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("round"))?,
            )?,
            value: Block::from_any(Any::decode(
                value
                    .value
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?
                    .as_slice(),
            )?)?,
            pol_round: Round::from_proto(
                proto
                    .pol_round
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("pol_round"))?,
            )?,
            validator_address: Address::from_proto(
                proto.validator_address.ok_or_else(|| {
                    proto::Error::missing_field::<Self::Proto>("validator_address")
                })?,
            )?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalContent {
    Tx(Tx),
    Proof(Proof),
}

impl proto::Protobuf for ProposalContent {
    type Proto = malachite_proto::Value;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(match self {
            ProposalContent::Tx(tx) => {
                let mut data = vec![1];
                data.extend_from_slice(&tx.data);
                proto::Value { value: Some(data) }
            }
            ProposalContent::Proof(proof) => {
                let mut data = vec![2];
                data.extend_from_slice(&proof.data);
                proto::Value { value: Some(data) }
            }
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let data = proto
            .value
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?;

        match data[0] {
            1 => Ok(ProposalContent::Tx(Tx {
                data: data[1..].to_vec(),
            })),
            2 => Ok(ProposalContent::Proof(Proof {
                data: data[1..].to_vec(),
            })),
            _ => Err(proto::Error::Other(
                "Invalid proposal content type".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tx {
    pub data: Vec<u8>,
}

impl Tx {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl proto::Protobuf for Tx {
    type Proto = malachite_proto::Tx;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self { data: proto.data })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(malachite_proto::Tx {
            data: self.data.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    pub data: Vec<u8>,
}

impl Proof {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl proto::Protobuf for Proof {
    type Proto = malachite_proto::Proof;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self { data: proto.data })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(malachite_proto::Proof {
            data: self.data.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Proposal(Proposal),
    Vote(Vote),
}

impl Message {
    pub fn hash(&self) -> MessageHash {
        MessageHash::new(Sha256::digest(self.to_bytes()).into())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Message::Proposal(proposal) => proposal.to_bytes(),
            Message::Vote(vote) => vote.to_bytes(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SignedMessage {
    pub message: Message,
    pub signature: Signature,
}
