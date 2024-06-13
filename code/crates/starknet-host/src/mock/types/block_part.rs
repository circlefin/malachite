use std::sync::Arc;

use signature::Signer;

use malachite_common::{Round, SignedBlockPart};
use malachite_proto::{self as proto};

use crate::mock::context::MockContext;
use crate::mock::types::{Address, BlockHash, Height, PrivateKey, ProposalPart, StarknetContext};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockMetadata {
    pub proof: Vec<u8>,
    pub hash: BlockHash,
}

impl BlockMetadata {
    pub fn new(proof: Vec<u8>, value: BlockHash) -> Self {
        Self { proof, hash: value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        proto::Protobuf::to_bytes(self).unwrap()
    }

    pub fn size_bytes(&self) -> usize {
        self.proof.len() + self.hash.len()
    }
}

impl proto::Protobuf for BlockMetadata {
    type Proto = crate::proto::mock::BlockMetadata;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self {
            proof: proto.proof,
            hash: BlockHash::from_proto(
                proto
                    .hash
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("hash"))?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(crate::proto::mock::BlockMetadata {
            proof: self.proof.clone(),
            hash: self.hash.to_proto().ok(),
        })
    }
}

/// A part of a value for a height, round. Identified in this scope by the sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockPart {
    pub height: Height,
    pub round: Round,
    pub sequence: u64,
    pub part: Arc<ProposalPart>,
    pub validator_address: Address,
}

impl BlockPart {
    pub fn new(
        height: Height,
        round: Round,
        sequence: u64,
        validator_address: Address,
        part: ProposalPart,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            part: Arc::new(part),
            validator_address,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        proto::Protobuf::to_bytes(self).unwrap()
    }

    pub fn signed(self, private_key: &PrivateKey) -> SignedBlockPart<StarknetContext> {
        let signature = private_key.sign(&self.to_bytes());

        SignedBlockPart {
            block_part: self,
            signature,
        }
    }

    pub fn metadata(&self) -> Option<&BlockMetadata> {
        match self.part.as_ref() {
            ProposalPart::Metadata(_, metadata) => Some(metadata),
            _ => None,
        }
    }

    pub fn tx_count(&self) -> Option<usize> {
        match self.part.as_ref() {
            ProposalPart::TxBatch(_, batch) => Some(batch.transactions().len()),
            _ => None,
        }
    }

    pub fn size_bytes(&self) -> usize {
        match self.part.as_ref() {
            ProposalPart::TxBatch(_, batch) => batch.size_bytes(),
            ProposalPart::Metadata(_, metadata) => metadata.size_bytes(),
        }
    }
}

impl malachite_common::BlockPart<MockContext> for BlockPart {
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
            sequence: proto.sequence,
            part: Arc::new(ProposalPart::from_any(&proto.content.ok_or_else(
                || proto::Error::missing_field::<Self::Proto>("content"),
            )?)?),
            validator_address: Address::from_proto(
                proto.validator_address.ok_or_else(|| {
                    proto::Error::missing_field::<Self::Proto>("validator_address")
                })?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::BlockPart {
            height: Some(self.height.to_proto()?),
            round: Some(self.round.to_proto()?),
            sequence: self.sequence,
            content: Some(self.part.to_any()?),
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }
}
