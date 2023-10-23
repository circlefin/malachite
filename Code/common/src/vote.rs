use core::fmt::Debug;

use crate::{Consensus, Round, Value};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

pub trait Vote<C: Consensus>
where
    Self: Clone + Debug + PartialEq + Eq,
{
    fn round(&self) -> Round;
    fn value(&self) -> Option<&<C::Value as Value>::Id>;
    fn vote_type(&self) -> VoteType;

    // FIXME: round message votes should not include address
    fn address(&self) -> &C::Address;
    fn set_address(&mut self, address: C::Address);
}
