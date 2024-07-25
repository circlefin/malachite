use malachite_common::Round;
use malachite_proto as proto;
use malachite_starknet_p2p_proto as p2p_proto;

use crate::{Address, BlockProof, Height, Transactions};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalMessage {
    Init(ProposalInit),
    Transactions(Transactions),
    BlockProof(BlockProof),
    Fin(ProposalFin),
}

impl ProposalMessage {
    pub fn message_type(&self) -> MessageType {
        match self {
            Self::Init(_) => MessageType::Init,
            Self::Transactions(_) => MessageType::Transactions,
            Self::BlockProof(_) => MessageType::BlockProof,
            Self::Fin(_) => MessageType::Fin,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MessageType {
    Init,
    Transactions,
    BlockProof,
    Fin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalPart {
    pub height: Height,
    pub round: Round,
    pub sequence: u64,
    pub validator: Address,
    pub message: ProposalMessage,
}

impl ProposalPart {
    pub fn init(
        height: Height,
        round: Round,
        sequence: u64,
        validator: Address,
        init: ProposalInit,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            validator,
            message: ProposalMessage::Init(init),
        }
    }

    pub fn transactions(
        height: Height,
        round: Round,
        sequence: u64,
        validator: Address,
        transactions: Transactions,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            validator,
            message: ProposalMessage::Transactions(transactions),
        }
    }

    pub fn block_proof(
        height: Height,
        round: Round,
        sequence: u64,
        validator: Address,
        block_proof: BlockProof,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            validator,
            message: ProposalMessage::BlockProof(block_proof),
        }
    }

    pub fn fin(
        height: Height,
        round: Round,
        sequence: u64,
        validator: Address,
        fin: ProposalFin,
    ) -> Self {
        Self {
            height,
            round,
            sequence,
            validator,
            message: ProposalMessage::Fin(fin),
        }
    }

    pub fn message_type(&self) -> MessageType {
        self.message.message_type()
    }

    pub fn to_sign_bytes(&self) -> Vec<u8> {
        proto::Protobuf::to_bytes(self).unwrap() // FIXME: unwrap
    }

    pub fn size_bytes(&self) -> usize {
        self.to_sign_bytes().len() // FIXME: Do something more efficient
    }

    pub fn tx_count(&self) -> usize {
        match &self.message {
            ProposalMessage::Transactions(txes) => txes.len(),
            _ => 0,
        }
    }
}

impl proto::Protobuf for ProposalPart {
    type Proto = p2p_proto::ProposalPart;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        use p2p_proto::proposal_part::Messages;

        let message = proto
            .messages
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("messages"))?;

        let message = match message {
            Messages::Init(init) => ProposalMessage::Init(ProposalInit {
                block_number: init.block_number,
                fork_id: init.fork_id,
                proposal_round: Round::new(i64::from(init.proposal_round)),
            }),
            Messages::Fin(fin) => {
                let valid_round = fin.valid_round.map(|round| Round::new(i64::from(round)));
                ProposalMessage::Fin(ProposalFin { valid_round })
            }
            Messages::Transactions(txes) => {
                let transactions = Transactions::from_proto(txes)?;
                ProposalMessage::Transactions(transactions)
            }
            Messages::Proof(proof) => {
                let block_proof = BlockProof::from_proto(proof)?;
                ProposalMessage::BlockProof(block_proof)
            }
        };

        let validator = Address::from_proto(
            proto
                .validator
                .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("validator"))?,
        )?;

        Ok(Self {
            height: Height::new(proto.height),
            round: Round::new(i64::from(proto.round)),
            sequence: proto.sequence,
            validator,
            message,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        use p2p_proto::proposal_part::Messages;

        let message = match &self.message {
            ProposalMessage::Init(init) => Messages::Init(p2p_proto::ProposalInit {
                block_number: init.block_number,
                fork_id: init.fork_id,
                proposal_round: init.proposal_round.as_i64() as u32, // FIXME: p2p-types
            }),
            ProposalMessage::Fin(fin) => Messages::Fin(p2p_proto::ProposalFin {
                valid_round: fin.valid_round.map(|round| round.as_i64() as u32), // FIXME: p2p-types
            }),
            ProposalMessage::Transactions(txes) => {
                Messages::Transactions(p2p_proto::Transactions {
                    transactions: txes
                        .as_slice()
                        .iter()
                        .map(|tx| tx.to_proto())
                        .collect::<Result<Vec<_>, _>>()?,
                })
            }
            ProposalMessage::BlockProof(block_proof) => Messages::Proof(block_proof.to_proto()?),
        };

        Ok(p2p_proto::ProposalPart {
            height: self.height.as_u64(),
            round: self.round.as_i64() as u32, // FIXME: p2p-types
            sequence: self.sequence,
            validator: Some(self.validator.to_proto()?),
            messages: Some(message),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalInit {
    pub block_number: u64,
    pub fork_id: u64,
    pub proposal_round: Round,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalFin {
    pub valid_round: Option<Round>,
}
