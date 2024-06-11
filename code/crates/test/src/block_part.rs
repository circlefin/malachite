use std::sync::Arc;

use signature::Signer;

use malachite_common::{Round, SignedBlockPart, TransactionBatch};
use malachite_proto::{self as proto};

use crate::{Address, Height, PrivateKey, TestContext, Value};

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
        proto::Protobuf::to_bytes(self).unwrap()
    }

    pub fn size_bytes(&self) -> usize {
        self.proof.len() + self.value.size_bytes()
    }
}

impl proto::Protobuf for BlockMetadata {
    type Proto = crate::proto::BlockMetadata;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self {
            proof: proto.proof,
            value: Value::from_proto(
                proto
                    .value
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("height"))?,
            )?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(crate::proto::BlockMetadata {
            proof: self.proof.clone(),
            value: Option::from(self.value.to_proto().unwrap()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    TxBatch(TransactionBatch),
    Metadata(BlockMetadata),
}

impl Content {
    pub fn size_bytes(&self) -> usize {
        match self {
            Content::TxBatch(batch) => batch.size_bytes(),
            Content::Metadata(meta) => meta.size_bytes(),
        }
    }

    pub fn tx_count(&self) -> Option<usize> {
        match self {
            Content::TxBatch(batch) => Some(batch.transactions().len()),
            Content::Metadata(_) => None,
        }
    }
}

impl proto::Protobuf for Content {
    type Proto = crate::proto::Content;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let content = proto
            .value
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?;

        match content {
            crate::proto::content::Value::TxBatch(batch) => {
                TransactionBatch::from_proto(batch).map(Content::TxBatch)
            }
            crate::proto::content::Value::Metadata(metadata) => {
                BlockMetadata::from_proto(metadata).map(Content::Metadata)
            }
        }
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        match self {
            Content::TxBatch(batch) => Ok(crate::proto::Content {
                value: Some(crate::proto::content::Value::TxBatch(batch.to_proto()?)),
            }),
            Content::Metadata(metadata) => Ok(crate::proto::Content {
                value: Some(crate::proto::content::Value::Metadata(metadata.to_proto()?)),
            }),
        }
    }
}

/// A part of a value for a height, round. Identified in this scope by the sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockPart {
    pub height: Height,
    pub round: Round,
    pub sequence: u64,
    pub content: Arc<Content>,
    pub validator_address: Address,
}

impl BlockPart {
    pub fn new(
        height: Height,
        round: Round,
        sequence: u64,
        validator_address: Address,
        content: Content,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            content: Arc::new(content),
            validator_address,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        proto::Protobuf::to_bytes(self).unwrap()
    }

    pub fn signed(self, private_key: &PrivateKey) -> SignedBlockPart<TestContext> {
        let signature = private_key.sign(&self.to_bytes());

        SignedBlockPart {
            block_part: self,
            signature,
        }
    }

    pub fn metadata(&self) -> Option<&BlockMetadata> {
        match self.content.as_ref() {
            Content::Metadata(metadata) => Some(metadata),
            Content::TxBatch(_) => None,
        }
    }

    pub fn tx_count(&self) -> Option<usize> {
        self.content.tx_count()
    }

    pub fn size_bytes(&self) -> usize {
        self.content.size_bytes()
    }
}

impl malachite_common::BlockPart<TestContext> for BlockPart {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn sequence(&self) -> u64 {
        self.sequence
    }

    fn validator_address(&self) -> &Address {
        &self.validator_address
    }
}

impl proto::Protobuf for BlockPart {
    type Proto = proto::BlockPart;

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
            sequence: proto.sequence,
            content: Arc::new(Content::from_any(
                &proto
                    .content
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("content"))?,
            )?),
            validator_address: Address::from_proto(
                proto.validator_address.ok_or_else(|| {
                    proto::Error::missing_field::<Self::Proto>("validator_address")
                })?,
            )?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::BlockPart {
            height: Some(self.height.to_proto()?),
            round: Some(self.round.to_proto()?),
            sequence: self.sequence,
            content: Some(self.content.to_any()?),
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }
}
