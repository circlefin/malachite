use core::fmt::Debug;

/// Defines the requirements for the type of value to decide on.
pub trait Value
where
    Self: Clone + Debug + PartialEq + Eq + PartialOrd + Ord,
{
    /// The type of the ID of the value.
    /// Typically a representation of the value with a lower memory footprint.
    type Id: Clone + Debug + PartialEq + Eq + PartialOrd + Ord;

    /// The ID of the value.
    fn id(&self) -> Self::Id;

    /// Whether the value is valid.
    fn is_valid(&self) -> bool;
}
