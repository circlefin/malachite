use signature::Signer;

use malachite_common::{Round, SignedBlockPart, Transaction};
use malachite_proto::{self as proto};

use crate::{Address, Height, PrivateKey, TestContext};

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockPart {
    pub height: Height,
    pub round: Round,
    pub sequence: u64,
    pub transactions: Vec<Transaction>,
    pub validator_address: Address,
}

impl BlockPart {
    pub fn new(
        height: Height,
        round: Round,
        sequence: u64,
        transactions: Vec<Transaction>,
        validator_address: Address,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            transactions,
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
            transactions: proto
                .block_part
                .iter()
                .map(|t| Transaction::from_proto(t.clone()).unwrap())
                .collect(),
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
            block_part: self
                .transactions
                .iter()
                .map(|t| t.to_proto().unwrap())
                .collect(),
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }
}
