use core::fmt::Debug;

use malachite_proto::Protobuf;

use crate::{Context, NilOrVal, Round, Value};

/// A type of vote.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VoteType {
    /// Votes for values which validators observe are valid for a given round.
    Prevote,

    /// Votes to commit to a particular value for a given round.
    Precommit,
}

/// Defines the requirements for a vote.
///
/// Votes are signed messages from validators for a particular value which
/// include information about the validator signing it.
pub trait Vote<Ctx>
where
    Self: Protobuf + Clone + Debug + Eq + Ord + Send + Sync + 'static,
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
