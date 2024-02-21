use malachite_common::Round;
use malachite_proto::{self as proto};

use crate::{Height, TestContext, Value};

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub height: Height,
    pub round: Round,
    pub value: Value,
    pub pol_round: Round,
}

impl Proposal {
    pub fn new(height: Height, round: Round, value: Value, pol_round: Round) -> Self {
        Self {
            height,
            round,
            value,
            pol_round,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        proto::Protobuf::to_bytes(self).unwrap()
    }
}

impl malachite_common::Proposal<TestContext> for Proposal {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &Value {
        &self.value
    }

    fn pol_round(&self) -> Round {
        self.pol_round
    }
}

impl proto::Protobuf for Proposal {
    type Proto = malachite_proto::Proposal;

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::Proposal {
            height: Some(self.height.to_proto()?),
            round: Some(self.round.to_proto()?),
            value: Some(self.value.to_proto()?),
            pol_round: Some(self.pol_round.to_proto()?),
        })
    }

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
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
            value: Value::from_proto(
                proto
                    .value
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?,
            )?,
            pol_round: Round::from_proto(
                proto
                    .pol_round
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("pol_round"))?,
            )?,
        })
    }
}
