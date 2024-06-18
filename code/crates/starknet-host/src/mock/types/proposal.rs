use malachite_common::Round;
use malachite_proto as proto;

use crate::mock::types::{Address, Height, ProposalContent, StarknetContext};

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub height: Height,
    pub round: Round,
    pub value: ProposalContent,
    pub pol_round: Round,
    pub validator_address: Address,
}

impl Proposal {
    pub fn new(
        height: Height,
        round: Round,
        value: ProposalContent,
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

    pub fn to_bytes(&self) -> Vec<u8> {
        proto::Protobuf::to_bytes(self).unwrap()
    }
}

impl malachite_common::Proposal<StarknetContext> for Proposal {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &ProposalContent {
        &self.value
    }

    fn pol_round(&self) -> Round {
        self.pol_round
    }

    fn validator_address(&self) -> &Address {
        &self.validator_address
    }
}

impl proto::Protobuf for Proposal {
    type Proto = malachite_proto::Proposal;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::Proposal {
            height: Some(self.height.to_proto()?),
            round: Some(self.round.to_proto()?),
            value: Some(proto::Value {
                value: Some(self.value.to_bytes()?),
            }),
            pol_round: Some(self.pol_round.to_proto()?),
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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
            value: ProposalContent::from_bytes(
                &proto
                    .value
                    .and_then(|v| v.value)
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?,
            )?,
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
