use crate::{Height, TestContext};
use malachite_common::{Round, Transaction};
use malachite_proto::{self as proto};

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockPart {
    pub height: Height,
    pub round: Round,
    pub sequence: u64,
    pub transactions: Vec<Transaction>,
}

impl BlockPart {
    pub fn new(
        height: Height,
        round: Round,
        sequence: u64,
        transactions: Vec<Transaction>,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            transactions,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        proto::Protobuf::to_bytes(self).unwrap()
    }
}

impl malachite_common::BlockPart<TestContext> for BlockPart {
    fn part_sequence(&self) -> u64 {
        self.sequence
    }

    fn part_height(&self) -> Height {
        self.height
    }

    fn part_round(&self) -> Round {
        self.round
    }
}

impl proto::Protobuf for BlockPart {
    type Proto = malachite_proto::BlockPart;

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
            transactions: proto
                .block_part
                .iter()
                .map(|t| Transaction::from_proto(t.clone()).unwrap())
                .collect(),
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::BlockPart {
            height: Some(self.height.to_proto()?),
            round: Some(self.round.to_proto()?),
            sequence: self.sequence,
            block_part: self
                .transactions
                .iter()
                .map(|t| t.to_proto().unwrap())
                .collect(),
        })
    }
}
