use malachite_common::{NilOrVal, Round, VoteType};
use malachite_proto::{Error as ProtoError, Protobuf};

use crate::proto;
use crate::{Address, Height, TestContext, ValueId};

/// A vote for a value in a round
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vote {
    pub typ: VoteType,
    pub height: Height,
    pub round: Round,
    pub value: NilOrVal<ValueId>,
    pub validator_address: Address,
}

impl Vote {
    pub fn new_prevote(
        height: Height,
        round: Round,
        value: NilOrVal<ValueId>,
        validator_address: Address,
    ) -> Self {
        Self {
            typ: VoteType::Prevote,
            height,
            round,
            value,
            validator_address,
        }
    }

    pub fn new_precommit(
        height: Height,
        round: Round,
        value: NilOrVal<ValueId>,
        address: Address,
    ) -> Self {
        Self {
            typ: VoteType::Precommit,
            height,
            round,
            value,
            validator_address: address,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        Protobuf::to_bytes(self).unwrap()
    }
}

impl malachite_common::Vote<TestContext> for Vote {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &NilOrVal<ValueId> {
        &self.value
    }

    fn take_value(self) -> NilOrVal<ValueId> {
        self.value
    }

    fn vote_type(&self) -> VoteType {
        self.typ
    }

    fn validator_address(&self) -> &Address {
        &self.validator_address
    }
}

impl Protobuf for Vote {
    type Proto = crate::proto::Vote;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            typ: decode_votetype(proto.vote_type()),
            height: Height::from_proto(
                proto
                    .height
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("height"))?,
            )?,
            round: Round::new(proto.round),
            value: match proto.value {
                Some(value) => NilOrVal::Val(ValueId::from_proto(value)?),
                None => NilOrVal::Nil,
            },
            validator_address: Address::from_proto(
                proto
                    .validator_address
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("validator_address"))?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            vote_type: encode_votetype(self.typ).into(),
            height: Some(self.height.to_proto()?),
            round: self.round.as_i64(),
            value: match &self.value {
                NilOrVal::Nil => None,
                NilOrVal::Val(v) => Some(v.to_proto()?),
            },
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn encode_votetype(vote_type: VoteType) -> proto::VoteType {
    match vote_type {
        VoteType::Prevote => proto::VoteType::Prevote,
        VoteType::Precommit => proto::VoteType::Precommit,
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn decode_votetype(vote_type: proto::VoteType) -> VoteType {
    match vote_type {
        proto::VoteType::Prevote => VoteType::Prevote,
        proto::VoteType::Precommit => VoteType::Precommit,
    }
}
