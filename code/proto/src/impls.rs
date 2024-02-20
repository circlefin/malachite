use core::convert::Infallible;

use malachite_common::{Context, Round, SignedVote, SigningScheme, VoteType};

use crate::{self as proto, Error, Protobuf};

impl TryFrom<proto::Round> for Round {
    type Error = Infallible;

    fn try_from(proto: proto::Round) -> Result<Self, Self::Error> {
        Ok(Self::new(proto.round))
    }
}

impl From<Round> for proto::Round {
    fn from(round: Round) -> proto::Round {
        proto::Round {
            round: round.as_i64(),
        }
    }
}

impl<Ctx: Context> From<SignedVote<Ctx>> for proto::SignedVote
where
    Ctx::Vote: Into<proto::Vote>,
{
    fn from(signed_vote: SignedVote<Ctx>) -> proto::SignedVote {
        proto::SignedVote {
            vote: Some(signed_vote.vote.into()),
            signature: Ctx::SigningScheme::encode_signature(&signed_vote.signature),
        }
    }
}

impl<Ctx: Context> TryFrom<proto::SignedVote> for SignedVote<Ctx>
where
    Ctx::Vote: TryFrom<proto::Vote, Error = Error>,
{
    type Error = Error;

    fn try_from(value: proto::SignedVote) -> Result<Self, Error> {
        let vote = value
            .vote
            .ok_or_else(|| Error::Other("Missing field `vote`".to_string()))?;

        Ok(Self {
            vote: Ctx::Vote::try_from(vote)?,
            signature: Ctx::SigningScheme::decode_signature(&value.signature)
                .map_err(|e| Error::Other(format!("Failed to decode signature: {e}")))?,
        })
    }
}

impl<Ctx: Context> Protobuf for SignedVote<Ctx>
where
    Ctx::Vote: TryFrom<proto::Vote, Error = Error> + Into<proto::Vote>,
{
    type Proto = proto::SignedVote;
}

impl TryFrom<proto::VoteType> for VoteType {
    type Error = Infallible;

    fn try_from(vote_type: proto::VoteType) -> Result<Self, Self::Error> {
        match vote_type {
            proto::VoteType::Prevote => Ok(VoteType::Prevote),
            proto::VoteType::Precommit => Ok(VoteType::Precommit),
        }
    }
}

impl From<VoteType> for proto::VoteType {
    fn from(vote_type: VoteType) -> proto::VoteType {
        match vote_type {
            VoteType::Prevote => proto::VoteType::Prevote,
            VoteType::Precommit => proto::VoteType::Precommit,
        }
    }
}
