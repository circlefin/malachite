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
            .ok_or_else(|| Error::missing_field::<Self::Proto>("vote"))?;

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

impl<Ctx: Context> Protobuf for common::SignedProposalPart<Ctx>
where
    Ctx::ProposalPart: Protobuf,
{
    type Proto = SignedProposalPart;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        let proposal_part = proto
            .proposal_part
            .ok_or_else(|| Error::missing_field::<Self::Proto>("proposal_part"))?;

        Ok(Self {
            proposal_part: Ctx::ProposalPart::from_any(&proposal_part)?,
            signature: Ctx::SigningScheme::decode_signature(&proto.signature)
                .map_err(|e| Error::Other(format!("Failed to decode signature: {e}")))?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        Ok(SignedProposalPart {
            proposal_part: Some(self.proposal_part.to_any()?),
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
