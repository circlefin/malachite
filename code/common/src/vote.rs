use core::convert::Infallible;
use core::fmt::Debug;

use malachite_proto::Protobuf;

use crate::{Context, NilOrVal, Round, Value};

/// A type of vote.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoteType {
    /// Votes for values which validators observe are valid for a given round.
    Prevote,

    /// Votes to commit to a particular value for a given round.
    Precommit,
}

impl TryFrom<malachite_proto::VoteType> for VoteType {
    type Error = Infallible;

    fn try_from(vote_type: malachite_proto::VoteType) -> Result<Self, Self::Error> {
        match vote_type {
            malachite_proto::VoteType::Prevote => Ok(VoteType::Prevote),
            malachite_proto::VoteType::Precommit => Ok(VoteType::Precommit),
        }
    }
}

impl From<VoteType> for malachite_proto::VoteType {
    fn from(vote_type: VoteType) -> malachite_proto::VoteType {
        match vote_type {
            VoteType::Prevote => malachite_proto::VoteType::Prevote,
            VoteType::Precommit => malachite_proto::VoteType::Precommit,
        }
    }
}

/// Defines the requirements for a vote.
///
/// Votes are signed messages from validators for a particular value which
/// include information about the validator signing it.
pub trait Vote<Ctx>
where
    Self: Clone + Debug + Eq + Send + Sync + 'static,
    Self: Protobuf<Proto = malachite_proto::Vote>,
    Ctx: Context,
{
    /// The height for which the vote is for.
    fn height(&self) -> Ctx::Height;

    /// The round for which the vote is for.
    fn round(&self) -> Round;

    /// Get a reference to the value being voted for.
    fn value(&self) -> &NilOrVal<<Ctx::Value as Value>::Id>;

    /// Take ownership of the value being voted for.
    fn take_value(self) -> NilOrVal<<Ctx::Value as Value>::Id>;

    /// The type of vote.
    fn vote_type(&self) -> VoteType;

    /// Address of the validator who issued this vote
    fn validator_address(&self) -> &Ctx::Address;
}
