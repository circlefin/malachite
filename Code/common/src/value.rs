use core::fmt::Debug;

pub trait Value
where
    Self: Clone + Debug + PartialEq + Eq + PartialOrd + Ord,
{
    type Id: Clone + Debug + PartialEq + Eq + PartialOrd + Ord;

    fn id(&self) -> Self::Id;

    fn valid(&self) -> bool;
}
