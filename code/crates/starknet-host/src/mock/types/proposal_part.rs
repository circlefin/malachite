use malachite_proto as proto;

use crate::mock::types::block_part::BlockMetadata;
use crate::mock::types::TransactionBatch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalPart {
    TxBatch(u64, TransactionBatch),
    Metadata(u64, BlockMetadata),
}

impl ProposalPart {
    pub fn sequence(&self) -> u64 {
        match self {
            Self::TxBatch(sequence, _) => *sequence,
            Self::Metadata(sequence, _) => *sequence,
        }
    }
}

impl proto::Protobuf for ProposalPart {
    type Proto = crate::proto::mock::ProposalPart;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        use crate::proto::mock::proposal_part::Part;

        let part = proto
            .part
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("part"))?;

        match part {
            Part::TxBatch(tx_batch) => Ok(Self::TxBatch(
                proto.sequence,
                TransactionBatch::from_proto(tx_batch)?,
            )),
            Part::Metadata(metadata) => Ok(Self::Metadata(
                proto.sequence,
                BlockMetadata::from_proto(metadata)?,
            )),
        }
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        use crate::proto::mock::proposal_part::Part;

        Ok(Self::Proto {
            sequence: self.sequence(),
            part: match self {
                Self::TxBatch(_, tx_batch) => Some(Part::TxBatch(tx_batch.to_proto()?)),
                Self::Metadata(_, metadata) => Some(Part::Metadata(metadata.to_proto()?)),
            },
        })
    }
}
