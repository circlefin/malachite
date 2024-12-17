use bytes::Bytes;
use serde::{Deserialize, Serialize};

use malachite_core_types::Round;
use malachite_proto::{Error as ProtoError, Protobuf};

use crate::{Address, Height, TestContext, Value};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Content {
    pub value: Value,
}

impl Content {
    pub fn new(value: Value) -> Self {
        Self { value }
    }

    pub fn size_bytes(&self) -> usize {
        self.value.size_bytes()
    }
}

impl Protobuf for Content {
    type Proto = crate::proto::Content;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            value: Value::from_proto(
                proto
                    .value
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("value"))?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            value: Some(self.value.to_proto()?),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Round")]
pub enum RoundDef {
    /// No round, ie. `-1`
    Nil,

    /// Some round `r` where `r >= 0`
    Some(u32),
}

/// A part of a value for a height, round. Identified in this scope by the sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalPart {
    pub height: Height,
    #[serde(with = "RoundDef")]
    pub round: Round,
    pub sequence: u64,
    pub content: Content,
    pub proposer: Address,
    pub fin: bool,
}

impl ProposalPart {
    pub fn new(
        height: Height,
        round: Round,
        sequence: u64,
        proposer: Address,
        content: Content,
        fin: bool,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            content,
            proposer,
            fin,
        }
    }

    pub fn to_bytes(&self) -> Bytes {
        Protobuf::to_bytes(self).unwrap()
    }

    pub fn size_bytes(&self) -> usize {
        self.content.size_bytes()
    }
}

impl malachite_core_types::ProposalPart<TestContext> for ProposalPart {
    fn is_first(&self) -> bool {
        self.sequence == 0
    }

    fn is_last(&self) -> bool {
        self.fin
    }
}

impl Protobuf for ProposalPart {
    type Proto = crate::proto::ProposalPart;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            height: Height::from_proto(proto.height)?,
            round: Round::new(proto.round),
            sequence: proto.sequence,
            content: Content::from_proto(
                proto
                    .content
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("content"))?,
            )?,
            proposer: Address::from_proto(
                proto
                    .validator_address
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("validator_address"))?,
            )?,
            fin: proto.fin,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(crate::proto::ProposalPart {
            height: self.height.to_proto()?,
            round: self.round.as_u32().expect("round should not be nil"),
            sequence: self.sequence,
            content: Some(self.content.to_proto()?),
            validator_address: Some(self.proposer.to_proto()?),
            fin: self.fin,
        })
    }
}
