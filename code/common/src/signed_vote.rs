use alloc::string::ToString;

use derive_where::derive_where;
use malachite_proto::Protobuf;

use crate::{Context, Signature, SigningScheme, Vote};

/// A signed vote, ie. a vote emitted by a validator and signed by its private key.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct SignedVote<Ctx>
where
    Ctx: Context,
{
    /// The vote.
    pub vote: Ctx::Vote,

    /// The signature of the vote.
    pub signature: Signature<Ctx>,
}

impl<Ctx> SignedVote<Ctx>
where
    Ctx: Context,
{
    /// Create a new signed vote from the given vote and signature.
    pub fn new(vote: Ctx::Vote, signature: Signature<Ctx>) -> Self {
        Self { vote, signature }
    }

    /// Return the address of the validator that emitted this vote.
    pub fn validator_address(&self) -> &Ctx::Address {
        self.vote.validator_address()
    }
}

use malachite_proto::Error as ProtoError;

impl<Ctx: Context> TryFrom<malachite_proto::SignedVote> for SignedVote<Ctx>
where
    Ctx::Vote: TryFrom<malachite_proto::Vote, Error = ProtoError>,
{
    type Error = malachite_proto::Error;

    fn try_from(value: malachite_proto::SignedVote) -> Result<Self, Self::Error> {
        let vote = value
            .vote
            .ok_or_else(|| ProtoError::Other("Missing field `vote`".to_string()))?;

        Ok(Self {
            vote: Ctx::Vote::try_from(vote)?,
            signature: Ctx::SigningScheme::decode_signature(&value.signature)?,
        })
    }
}

impl<Ctx: Context> Protobuf for SignedVote<Ctx> {
    type Proto = malachite_proto::SignedVote;
}
