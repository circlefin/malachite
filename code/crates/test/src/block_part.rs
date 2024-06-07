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
    type Proto = proto::BlockMetadata;

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
        Ok(proto::BlockMetadata {
            proof: self.proof.clone(),
            value: Option::from(self.value.to_proto().unwrap()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub transaction_batch: TransactionBatch,
    pub block_metadata: Option<BlockMetadata>,
}

impl Content {
    pub fn new(transaction_batch: TransactionBatch, block_metadata: Option<BlockMetadata>) -> Self {
        Self {
            transaction_batch,
            block_metadata,
        }
    }

    pub fn size_bytes(&self) -> usize {
        let txes_size = self
            .transaction_batch
            .transactions()
            .iter()
            .map(|tx| tx.size_bytes())
            .sum::<usize>();

        let meta_size = self
            .block_metadata
            .as_ref()
            .map(|meta| meta.to_bytes().len())
            .unwrap_or(0);

        txes_size + meta_size
    }
}

impl proto::Protobuf for Content {
    type Proto = proto::Content;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let block_metadata = match proto.metadata {
            Some(meta) => Some(BlockMetadata::from_proto(meta)?),
            None => None,
        };

        Ok(Content {
            transaction_batch: TransactionBatch::from_proto(proto.tx_batch.unwrap())?,
            block_metadata,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        //TODO fix
        let metadata = match self.block_metadata.clone() {
            Some(meta) => Some(meta.to_proto()?),
            None => None,
        };
        Ok(proto::Content {
            tx_batch: Some(self.transaction_batch.to_proto()?),
            metadata,
        })
    }
}

/// A part of a value for a height, round. Identified in this scope by the sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockPart {
    pub height: Height,
    pub round: Round,
    pub sequence: u64,
    pub content: Content,
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
            content,
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

    pub fn metadata(&self) -> Option<BlockMetadata> {
        self.content.block_metadata.clone()
    }

    pub fn size_bytes(&self) -> usize {
        self.content.size_bytes()
    }

    pub fn tx_count(&self) -> usize {
        self.content.transaction_batch.len()
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
            content: Content::from_proto(
                proto
                    .content
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("content"))?,
            )?,
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
            content: Some(self.content.to_proto()?),
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }
}
