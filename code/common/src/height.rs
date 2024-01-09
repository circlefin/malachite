use core::fmt::Debug;

/// Defines the requirements for a height type.
///
/// A height denotes the number of blocks (values) created since the chain began.
///
/// A height of 0 represents a chain which has not yet produced a block.
pub trait Height
where
    Self: Default + Copy + Clone + Debug + PartialEq + Eq + PartialOrd + Ord,
{
}
