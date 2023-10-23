use core::fmt::Debug;

// TODO: Keep the trait or just add the bounds to Consensus::Height?
pub trait Height
where
    // TODO: Require Copy as well?
    Self: Clone + Debug + PartialEq + Eq + PartialOrd + Ord,
{
}
