//! Protobuf instances for common::common types

#![allow(missing_docs)]

use alloc::format;
use alloc::string::ToString;

pub use malachite_proto::{Error, Protobuf};

use crate::{self as common, Context, SigningScheme};

include!(concat!(env!("OUT_DIR"), "/malachite.common.rs"));

impl Protobuf for common::Round {
    type Proto = Round;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        Ok(Self::new(proto.round))
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        Ok(Round {
            round: self.as_i64(),
        })
    }
}

impl<Ctx: Context> Protobuf for common::SignedVote<Ctx>
where
    Ctx::Vote: Protobuf,
{
    type Proto = SignedVote;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        let vote = proto
            .vote
            .ok_or_else(|| Error::missing_field::<SignedVote>("vote"))?;

        Ok(Self {
            vote: Ctx::Vote::from_any(&vote)?,
            signature: Ctx::SigningScheme::decode_signature(&proto.signature)
                .map_err(|e| Error::Other(format!("Failed to decode signature: {e}")))?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        Ok(SignedVote {
            vote: Some(self.vote.to_any()?),
            signature: Ctx::SigningScheme::encode_signature(&self.signature),
        })
    }
}

impl<Ctx: Context> Protobuf for common::SignedBlockPart<Ctx>
where
    Ctx::BlockPart: Protobuf,
{
    type Proto = SignedBlockPart;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        let block_part = proto
            .block_part
            .ok_or_else(|| Error::missing_field::<BlockPart>("block_part"))?;

        Ok(Self {
            block_part: Ctx::BlockPart::from_any(&block_part)?,
            signature: Ctx::SigningScheme::decode_signature(&proto.signature)
                .map_err(|e| Error::Other(format!("Failed to decode signature: {e}")))?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        Ok(SignedBlockPart {
            block_part: Some(self.block_part.to_any()?),
            signature: Ctx::SigningScheme::encode_signature(&self.signature),
        })
    }
}

impl From<VoteType> for common::VoteType {
    fn from(vote_type: VoteType) -> Self {
        match vote_type {
            VoteType::Prevote => common::VoteType::Prevote,
            VoteType::Precommit => common::VoteType::Precommit,
        }
    }
}

impl From<common::VoteType> for VoteType {
    fn from(vote_type: common::VoteType) -> VoteType {
        match vote_type {
            common::VoteType::Prevote => VoteType::Prevote,
            common::VoteType::Precommit => VoteType::Precommit,
        }
    }
}

impl<Ctx: Context> Protobuf for common::SignedProposal<Ctx>
where
    Ctx::Proposal: Protobuf,
{
    type Proto = SignedProposal;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        let proposal = proto
            .proposal
            .ok_or_else(|| Error::Other("Missing field `proposal`".to_string()))?;

        Ok(Self {
            proposal: Ctx::Proposal::from_any(&proposal)?,
            signature: Ctx::SigningScheme::decode_signature(&proto.signature)
                .map_err(|e| Error::Other(format!("Failed to decode signature: {e}")))?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        Ok(SignedProposal {
            proposal: Some(self.proposal.to_any()?),
            signature: Ctx::SigningScheme::encode_signature(&self.signature),
        })
    }
}

impl Protobuf for common::Transaction {
    type Proto = Transaction;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        let tx = proto
            .value
            .ok_or_else(|| Error::Other("Missing field `value`".to_string()))?;

        Ok(Self::new(tx))
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        let value = self.to_bytes();
        Ok(Transaction { value: Some(value) })
    }
}

impl Protobuf for common::TransactionBatch {
    type Proto = TransactionBatch;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        Ok(Self::new(
            proto
                .transactions
                .into_iter()
                .map(common::Transaction::from_proto)
                .collect::<Result<_, _>>()?,
        ))
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        Ok(TransactionBatch {
            transactions: self
                .transactions()
                .iter()
                .map(|t| t.to_proto())
                .collect::<Result<_, _>>()?,
        })
    }
}

impl Protobuf for common::MempoolTransactionBatch {
    type Proto = MempoolTransactionBatch;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        Ok(Self::new(common::TransactionBatch::from_proto(
            proto
                .transaction_batch
                .ok_or_else(|| Error::missing_field::<Self::Proto>("content"))?,
        )?))
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        Ok(MempoolTransactionBatch {
            transaction_batch: Some(self.transaction_batch.to_proto()?),
        })
    }
}
