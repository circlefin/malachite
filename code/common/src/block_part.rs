use crate::{Context, Round};
use core::fmt::Debug;

/// Defines the requirements for a block part type.

pub trait BlockPart<Ctx>
where
    Self: Debug + Eq + Send + Sync + 'static,
    Ctx: Context,
{
    /// The part sequence
    fn part_sequence(&self) -> u64;

    /// The part height
    fn part_height(&self) -> Ctx::Height;

    /// The part round
    fn part_round(&self) -> Round;
}
