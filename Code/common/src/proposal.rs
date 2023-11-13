use core::fmt::Debug;

use crate::{Context, Round};

/// Defines the requirements for a proposal type.
pub trait Proposal<Ctx>
where
    Self: Clone + Debug + PartialEq + Eq,
    Ctx: Context,
{
    /// The height for which the proposal is for.
    fn height(&self) -> Ctx::Height;

    /// The round for which the proposal is for.
    fn round(&self) -> Round;

    /// The value that is proposed.
    fn value(&self) -> &Ctx::Value;

    /// The POL round for which the proposal is for.
    fn pol_round(&self) -> Round;
}
