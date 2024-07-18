use malachite_common::{self as common};
use malachite_proto as proto;

use crate::mock::types::block_part::BlockMetadata;
use crate::mock::types::BlockHash;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalContent {
    pub metadata: BlockMetadata,
}

impl PartialOrd for ProposalContent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProposalContent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.block_hash().cmp(&other.block_hash())
    }
}

impl ProposalContent {
    pub fn new(metadata: BlockMetadata) -> Self {
        Self { metadata }
    }

    pub fn block_hash(&self) -> BlockHash {
        self.metadata.hash
    }
}

impl common::Value for ProposalContent {
    type Id = BlockHash;

    fn id(&self) -> Self::Id {
        self.block_hash()
    }
}

impl proto::Protobuf for ProposalContent {
    type Proto = crate::proto::mock::ProposalContent;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let metadata = proto
            .metadata
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("metadata"))?;

        Ok(Self {
            metadata: BlockMetadata::from_proto(metadata)?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(crate::proto::mock::ProposalContent {
            metadata: Some(self.metadata.to_proto()?),
        })
    }
}
