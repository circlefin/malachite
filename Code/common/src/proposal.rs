use core::fmt::Debug;

use crate::{Consensus, Round};

/// Defines the requirements for a proposal type.
pub trait Proposal<C: Consensus>
where
    Self: Clone + Debug + PartialEq + Eq,
{
    /// The height for which the proposal is for.
    fn height(&self) -> C::Height;

    /// The round for which the proposal is for.
    fn round(&self) -> Round;

    /// The value that is proposed.
    fn value(&self) -> &C::Value;

    /// The POL round for which the proposal is for.
    fn pol_round(&self) -> Round;
}
