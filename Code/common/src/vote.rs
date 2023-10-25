use core::fmt::Debug;

use crate::{Consensus, Round, Value};

/// A type of vote.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
pub trait Vote<C: Consensus>
where
    Self: Clone + Debug + PartialEq + Eq,
{
    /// The round for which the vote is for.
    fn round(&self) -> Round;

    /// The value being voted for.
    fn value(&self) -> Option<&<C::Value as Value>::Id>;

    /// The type of vote.
    fn vote_type(&self) -> VoteType;

    // FIXME: round message votes should not include address
    fn address(&self) -> &C::Address;
    fn set_address(&mut self, address: C::Address);
}
