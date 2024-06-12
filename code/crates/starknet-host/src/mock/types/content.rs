use malachite_common::{self as common};
use malachite_proto as proto;

use crate::mock::types::block_part::BlockMetadata;
use crate::mock::types::{BlockHash, TransactionBatch};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub tx_batch: TransactionBatch,
    pub metadata: BlockMetadata,
}

impl PartialOrd for Content {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Content {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.block_hash().cmp(&other.block_hash())
    }
}

impl Content {
    pub fn block_hash(&self) -> BlockHash {
        self.metadata.hash
    }

    pub fn size_bytes(&self) -> usize {
        self.tx_batch.size_bytes() + self.metadata.size_bytes()
    }

    pub fn tx_count(&self) -> usize {
        self.tx_batch.transactions().len()
    }
}

impl common::Value for Content {
    type Id = BlockHash;

    fn id(&self) -> Self::Id {
        self.metadata.hash
    }
}

impl proto::Protobuf for Content {
    type Proto = crate::proto::mock::Content;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let tx_batch = proto
            .tx_batch
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("tx_batch"))?;

        let metadata = proto
            .metadata
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("metadata"))?;

        Ok(Self {
            tx_batch: TransactionBatch::from_proto(tx_batch)?,
            metadata: BlockMetadata::from_proto(metadata)?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        todo!()
    }
}
