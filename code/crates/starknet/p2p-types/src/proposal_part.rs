use malachite_common::Round;
use malachite_proto as proto;
use malachite_starknet_p2p_proto as p2p_proto;

use crate::{Address, BlockProof, Height, Transactions};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalInit {
    pub height: Height,
    pub proposal_round: Round,
    pub proposer: Address,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalFin {
    pub valid_round: Option<Round>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalPart {
    Init(ProposalInit),
    Transactions(Transactions),
    BlockProof(BlockProof),
    Fin(ProposalFin),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PartType {
    Init,
    Transactions,
    BlockProof,
    Fin,
}

impl ProposalPart {
    pub fn part_type(&self) -> PartType {
        match self {
            Self::Init(_) => PartType::Init,
            Self::Transactions(_) => PartType::Transactions,
            Self::BlockProof(_) => PartType::BlockProof,
            Self::Fin(_) => PartType::Fin,
        }
    }
    pub fn to_sign_bytes(&self) -> Vec<u8> {
        proto::Protobuf::to_bytes(self).unwrap() // FIXME: unwrap
    }

    pub fn size_bytes(&self) -> usize {
        self.to_sign_bytes().len() // FIXME: Do something more efficient
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

    pub fn as_transactions(&self) -> Option<&Transactions> {
        if let Self::Transactions(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_block_proof(&self) -> Option<&BlockProof> {
        if let Self::BlockProof(v) = self {
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
                height: Height::new(init.block_number, init.fork_id),
                proposal_round: Round::new(i64::from(init.proposal_round)),
                proposer: Address::from_proto(
                    init.proposer
                        .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("proposer"))?,
                )?,
            }),
            Messages::Fin(fin) => {
                let valid_round = fin.valid_round.map(|round| Round::new(i64::from(round)));
                ProposalPart::Fin(ProposalFin { valid_round })
            }
            Messages::Transactions(txes) => {
                let transactions = Transactions::from_proto(txes)?;
                ProposalPart::Transactions(transactions)
            }
            Messages::Proof(proof) => {
                let block_proof = BlockProof::from_proto(proof)?;
                ProposalPart::BlockProof(block_proof)
            }
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        use p2p_proto::proposal_part::Messages;

        let message = match self {
            ProposalPart::Init(init) => Messages::Init(p2p_proto::ProposalInit {
                block_number: init.height.block_number,
                fork_id: init.height.fork_id,
                proposal_round: init.proposal_round.as_i64() as u32, // FIXME: p2p-types
                proposer: Some(init.proposer.to_proto()?),
            }),
            ProposalPart::Fin(fin) => Messages::Fin(p2p_proto::ProposalFin {
                valid_round: fin.valid_round.map(|round| round.as_i64() as u32), // FIXME: p2p-types
            }),
            ProposalPart::Transactions(txes) => Messages::Transactions(p2p_proto::Transactions {
                transactions: txes
                    .as_slice()
                    .iter()
                    .map(|tx| tx.to_proto())
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            ProposalPart::BlockProof(block_proof) => Messages::Proof(block_proof.to_proto()?),
        };

        Ok(p2p_proto::ProposalPart {
            messages: Some(message),
        })
    }
}
