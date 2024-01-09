use core::fmt::Debug;

// TODO: Keep the trait or just add the bounds to Consensus::Height?
/// Defines the requirements for a height type.
///
/// A height denotes the number of blocks (values) created since the chain began.
///
/// A height of 0 represents a chain which has not yet produced a block.
pub trait Height
where
    // TODO: Require Copy as well?
    Self: Default + Clone + Debug + PartialEq + Eq + PartialOrd + Ord,
{
}
