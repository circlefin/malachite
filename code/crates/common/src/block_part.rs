use core::fmt::Debug;

use malachite_proto::Protobuf;

use crate::{Context, Round};

/// Defines the requirements for a block part type.
pub trait BlockPart<Ctx>
where
    Self: Protobuf + Clone + Debug + Eq + Send + Sync + 'static,
    Ctx: Context,
{
    /// The part height
    fn height(&self) -> Ctx::Height;

    /// The part round
    fn round(&self) -> Round;

    /// The part sequence
    fn sequence(&self) -> u64;

    /// Address of the validator who created this block part
    fn validator_address(&self) -> &Ctx::Address;
}
