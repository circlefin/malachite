use core::fmt::Debug;

use crate::{Consensus, Round};

pub trait Proposal<C: Consensus>
where
    Self: Clone + Debug + PartialEq + Eq,
{
    fn height(&self) -> C::Height;
    fn round(&self) -> Round;
    fn value(&self) -> &C::Value;
    fn pol_round(&self) -> Round;
}
