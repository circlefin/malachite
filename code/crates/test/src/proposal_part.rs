use std::sync::Arc;

use malachite_common::Round;
use malachite_proto::{Error as ProtoError, Protobuf};

use crate::{Address, Height, TestContext, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockMetadata {
    proof: Vec<u8>,
    value: Value,
}

impl BlockMetadata {
    pub fn new(proof: Vec<u8>, value: Value) -> Self {
        Self { proof, value }
    }

    pub fn value(&self) -> Value {
        self.value
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        Protobuf::to_bytes(self).unwrap()
    }

    pub fn size_bytes(&self) -> usize {
        self.proof.len() + self.value.size_bytes()
    }
}

impl Protobuf for BlockMetadata {
    type Proto = crate::proto::BlockMetadata;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            proof: proto.proof,
            value: Value::from_proto(
                proto
                    .value
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("height"))?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(crate::proto::BlockMetadata {
            proof: self.proof.clone(),
            value: Option::from(self.value.to_proto().unwrap()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    metadata: BlockMetadata,
}

impl Content {
    pub fn size_bytes(&self) -> usize {
        self.metadata.size_bytes()
    }
}

impl Protobuf for Content {
    type Proto = crate::proto::Content;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            metadata: BlockMetadata::from_proto(
                proto
                    .metadata
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("metadata"))?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            metadata: Some(self.metadata.to_proto()?),
        })
    }
}

/// A part of a value for a height, round. Identified in this scope by the sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalPart {
    pub height: Height,
    pub round: Round,
    pub sequence: u64,
    pub content: Arc<Content>,
    pub validator_address: Address,
    pub fin: bool,
}

impl ProposalPart {
    pub fn new(
        height: Height,
        round: Round,
        sequence: u64,
        validator_address: Address,
        content: Content,
        fin: bool,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            content: Arc::new(content),
            validator_address,
            fin,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        Protobuf::to_bytes(self).unwrap()
    }

    pub fn metadata(&self) -> &BlockMetadata {
        &self.content.metadata
    }

    pub fn size_bytes(&self) -> usize {
        self.content.size_bytes()
    }
}

impl malachite_common::ProposalPart<TestContext> for ProposalPart {
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
            height: Height::from_proto(
                proto
                    .height
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("height"))?,
            )?,
            round: Round::new(proto.round),
            sequence: proto.sequence,
            content: Arc::new(Content::from_any(
                &proto
                    .content
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("content"))?,
            )?),
            validator_address: Address::from_proto(
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
            height: Some(self.height.to_proto()?),
            round: self.round.as_i64(),
            sequence: self.sequence,
            content: Some(self.content.to_any()?),
            validator_address: Some(self.validator_address.to_proto()?),
            fin: self.fin,
        })
    }
}
