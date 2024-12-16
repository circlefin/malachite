use core::fmt::Debug;

use crate::Context;

/// Defines the requirements for a proposal part type.
pub trait ProposalPart<Ctx>
where
    Self: Clone + Debug + Eq + Send + Sync + 'static,
    Ctx: Context,
{
    /// Is this the first proposal part?
    fn is_first(&self) -> bool;

    /// Is this the last proposal part?
    fn is_last(&self) -> bool;
}
