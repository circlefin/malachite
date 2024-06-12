use malachite_common::{NilOrVal, Round, VoteType};
use malachite_proto as proto;

use crate::mock::hash;

pub type StarknetContext = malachite_test::TestContext;

pub type Height = malachite_test::Height;
pub type Validator = malachite_test::Validator;
pub type ValidatorSet = malachite_test::ValidatorSet;
pub type Address = malachite_test::Address;
pub type SigningScheme = malachite_test::Ed25519;
pub type BlockPart = malachite_test::BlockPart;
pub type ProposalContent = malachite_test::Content;
pub type Hash = hash::Hash;
pub type MessageHash = hash::MessageHash;
pub type BlockHash = hash::BlockHash;
pub type Precommit = Vote;

pub type Signature = malachite_test::Signature;
pub type PublicKey = malachite_test::PublicKey;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote {
    pub vote_type: VoteType,
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
            vote_type: VoteType::from(proto.vote_type()),
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
            vote_type: proto::VoteType::from(self.vote_type).into(),
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
