use bytes::Bytes;
use malachitebft_core_types::Round;
use malachitebft_proto::{Error as ProtoError, Protobuf};

use super::proto;

use crate::types::{address::Address, context::MockContext, height::Height, value::Value};

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub height: Height,
    pub round: Round,
    pub value: Value,
    pub pol_round: Round,
    pub validator_address: Address,
}

impl Proposal {
    pub fn new(
        height: Height,
        round: Round,
        value: Value,
        pol_round: Round,
        validator_address: Address,
    ) -> Self {
        Self {
            height,
            round,
            value,
            pol_round,
            validator_address,
        }
    }

    pub fn to_bytes(&self) -> Bytes {
        Protobuf::to_bytes(self).unwrap()
    }

    pub fn to_sign_bytes(&self) -> Bytes {
        Protobuf::to_bytes(self).unwrap()
    }
}

impl malachitebft_core_types::Proposal<MockContext> for Proposal {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &Value {
        &self.value
    }

    fn take_value(self) -> Value {
        self.value
    }

    fn pol_round(&self) -> Round {
        self.pol_round
    }

    fn validator_address(&self) -> &Address {
        &self.validator_address
    }
}

impl Protobuf for Proposal {
    type Proto = proto::Proposal;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            height: self.height.to_proto()?,
            round: self.round.as_u32().expect("round should not be nil"),
            value: Some(self.value.to_proto()?),
            pol_round: self.pol_round.as_u32(),
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            height: Height::from_proto(proto.height)?,
            round: Round::new(proto.round),
            value: Value::from_proto(
                proto
                    .value
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("value"))?,
            )?,
            pol_round: Round::from(proto.pol_round),
            validator_address: Address::from_proto(
                proto
                    .validator_address
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("validator_address"))?,
            )?,
        })
    }
}
