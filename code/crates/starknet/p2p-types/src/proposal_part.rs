use bytes::Bytes;
use malachitebft_core_types::Round;
use malachitebft_proto as proto;
use malachitebft_starknet_p2p_proto::{self as p2p_proto};

use crate::{Address, BlockInfo, Hash, Height, ProposalCommitment, TransactionBatch};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalInit {
    pub height: Height,
    pub round: Round,
    pub valid_round: Round,
    pub proposer: Address,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalFin {
    pub proposal_commitment_hash: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalPart {
    Init(ProposalInit),
    BlockInfo(BlockInfo),
    Transactions(TransactionBatch),
    Commitment(Box<ProposalCommitment>),
    Fin(ProposalFin),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PartType {
    Init,
    BlockInfo,
    Transactions,
    ProposalCommitment,
    Fin,
}

impl ProposalPart {
    pub fn part_type(&self) -> PartType {
        match self {
            Self::Init(_) => PartType::Init,
            Self::BlockInfo(_) => PartType::BlockInfo,
            Self::Transactions(_) => PartType::Transactions,
            Self::Commitment(_) => PartType::ProposalCommitment,
            Self::Fin(_) => PartType::Fin,
        }
    }

    pub fn to_sign_bytes(&self) -> Bytes {
        proto::Protobuf::to_bytes(self).unwrap()
    }

    pub fn size_bytes(&self) -> usize {
        self.to_sign_bytes().len() // TODO: Do this more efficiently
    }

    pub fn tx_count(&self) -> usize {
        match self {
            Self::Transactions(txes) => txes.len(),
            _ => 0,
        }
    }

    pub fn as_init(&self) -> Option<&ProposalInit> {
        if let Self::Init(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_block_info(&self) -> Option<&BlockInfo> {
        if let Self::BlockInfo(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_transactions(&self) -> Option<&TransactionBatch> {
        if let Self::Transactions(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_commitment(&self) -> Option<&ProposalCommitment> {
        if let Self::Commitment(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_fin(&self) -> Option<&ProposalFin> {
        if let Self::Fin(v) = self {
            Some(v)
        } else {
            None
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

        Ok(match message {
            Messages::Init(init) => ProposalPart::Init(ProposalInit {
                height: Height::new(init.height, 0),
                round: Round::new(init.round),
                valid_round: init.valid_round.into(),
                proposer: Address::from_proto(
                    init.proposer
                        .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("proposer"))?,
                )?,
            }),

            Messages::BlockInfo(block_info) => {
                ProposalPart::BlockInfo(BlockInfo::from_proto(block_info)?)
            }

            Messages::Transactions(txes) => {
                let transactions = TransactionBatch::from_proto(txes)?;
                ProposalPart::Transactions(transactions)
            }

            Messages::Commitment(commitment) => {
                ProposalPart::Commitment(Box::new(ProposalCommitment::from_proto(commitment)?))
            }

            Messages::Fin(fin) => ProposalPart::Fin(ProposalFin {
                proposal_commitment_hash: Hash::from_proto(fin.proposal_commitment.ok_or_else(
                    || proto::Error::missing_field::<Self::Proto>("proposal_commitment"),
                )?)?,
            }),
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        use p2p_proto::proposal_part::Messages;

        let message = match self {
            ProposalPart::Init(init) => Messages::Init(p2p_proto::ProposalInit {
                height: init.height.block_number,
                round: init.round.as_u32().expect("round should not be nil"),
                valid_round: init.valid_round.as_u32(),
                proposer: Some(init.proposer.to_proto()?),
            }),
            ProposalPart::BlockInfo(block_info) => Messages::BlockInfo(block_info.to_proto()?),
            ProposalPart::Transactions(txes) => {
                Messages::Transactions(p2p_proto::TransactionBatch {
                    transactions: txes
                        .as_slice()
                        .iter()
                        .map(|tx| tx.to_proto())
                        .collect::<Result<Vec<_>, _>>()?,
                })
            }
            ProposalPart::Commitment(commitment) => Messages::Commitment(commitment.to_proto()?),
            ProposalPart::Fin(fin) => Messages::Fin(p2p_proto::ProposalFin {
                proposal_commitment: Some(fin.proposal_commitment_hash.to_proto()?),
            }),
        };

        Ok(p2p_proto::ProposalPart {
            messages: Some(message),
        })
    }
}
