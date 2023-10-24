use core::fmt::Debug;

use crate::{Consensus, Round, Value};

/// A type of vote.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

/// Defines the requirements for a vote.
///
/// A vote is a signed message that is sent by a validator to the network.
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
