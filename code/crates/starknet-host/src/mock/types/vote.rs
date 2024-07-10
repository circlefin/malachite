use malachite_common::{self as common, proto, NilOrVal, Round, VoteType};

use crate::mock::types::{Address, BlockHash, Height, StarknetContext};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote {
    pub vote_type: VoteType,
    pub height: Height,
    pub round: Round,
    pub value: NilOrVal<BlockHash>,
    pub validator_address: Address,
}

impl Vote {
    pub fn new_prevote(
        height: Height,
        round: Round,
        value: NilOrVal<BlockHash>,
        validator_address: Address,
    ) -> Self {
        Self {
            vote_type: VoteType::Prevote,
            height,
            round,
            value,
            validator_address,
        }
    }

    pub fn new_precommit(
        height: Height,
        round: Round,
        value: NilOrVal<BlockHash>,
        address: Address,
    ) -> Self {
        Self {
            vote_type: VoteType::Precommit,
            height,
            round,
            value,
            validator_address: address,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        malachite_proto::Protobuf::to_bytes(self).unwrap()
    }
}

impl common::Vote<StarknetContext> for Vote {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &NilOrVal<BlockHash> {
        &self.value
    }

    fn take_value(self) -> NilOrVal<BlockHash> {
        self.value
    }

    fn vote_type(&self) -> VoteType {
        self.vote_type
    }

    fn validator_address(&self) -> &Address {
        &self.validator_address
    }
}

impl proto::Protobuf for Vote {
    type Proto = crate::proto::mock::Vote;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self {
            vote_type: VoteType::from(proto.vote_type()),
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
            value: match proto.value {
                Some(value) => {
                    let value = value
                        .value
                        .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?;

                    NilOrVal::Val(BlockHash::from_bytes(&value)?)
                }
                None => NilOrVal::Nil,
            },
            validator_address: Address::from_proto(
                proto.validator_address.ok_or_else(|| {
                    proto::Error::missing_field::<Self::Proto>("validator_address")
                })?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(Self::Proto {
            vote_type: proto::VoteType::from(self.vote_type).into(),
            height: Some(self.height.to_proto()?),
            round: Some(self.round.to_proto()?),
            value: match &self.value {
                NilOrVal::Nil => None,
                NilOrVal::Val(v) => Some(proto::ValueId {
                    value: Some(v.to_bytes()?),
                }),
            },
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }
}
