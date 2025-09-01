use malachitebft_proto::{Error as ProtoError, Protobuf};
use prost::Message;
use serde::{Deserialize, Serialize};

use super::proto;
use crate::types::{hash::BlockHash, height::Height, transaction::TransactionBatch};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Block {
    pub height: Height,
    pub transactions: TransactionBatch,
    pub block_hash: BlockHash,
}

impl Block {
    pub fn new(height: Height, transactions: TransactionBatch, block_hash: BlockHash) -> Self {
        Self {
            height,
            transactions,
            block_hash,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtoError> {
        let proto = proto::Block::decode(bytes)?;
        Self::from_proto(proto)
    }

    /// The size of the block in bytes
    pub fn size_bytes(&self) -> usize {
        let tx_size = self.transactions.size_bytes();
        let block_hash_size = self.block_hash.to_vec().len();
        let height_size = std::mem::size_of::<u64>();
        tx_size + block_hash_size + height_size
    }
}

impl Protobuf for Block {
    type Proto = proto::Block;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        let transactions = proto
            .transactions
            .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("transactions"))?;

        let block_hash = proto
            .block_hash
            .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("block_hash"))?;

        Ok(Self {
            height: Height::new(proto.height),
            transactions: TransactionBatch::from_proto(transactions)?,
            block_hash: BlockHash::from_bytes(&block_hash.elements)
                .map_err(ProtoError::invalid_data::<Self::Proto>)?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            height: self.height.to_proto()?,
            transactions: Some(self.transactions.to_proto()?),
            block_hash: Some(proto::Hash {
                elements: self.block_hash.to_vec().into(),
            }),
        })
    }
}
