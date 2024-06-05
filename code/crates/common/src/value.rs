use core::fmt::Debug;

/// Represents either `Nil` or a value of type `Value`.
///
/// This type is isomorphic to `Option<Value>` but is more explicit about its intent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum NilOrVal<Value> {
    /// The value is `nil`.
    #[default]
    Nil,

    /// The value is a value of type `Value`.
    Val(Value),
}

impl<Value> NilOrVal<Value> {
    // /// Whether this is `nil`.
    // pub fn is_nil(&self) -> bool {
    //     matches!(self, Self::Nil)
    // }
    //
    // /// Whether this is an actual value.
    // pub fn is_val(&self) -> bool {
    //     matches!(self, Self::Val(_))
    // }

    /// Apply the given function to the value if it is not `nil`.
    pub fn map<NewValue, F: FnOnce(Value) -> NewValue>(self, f: F) -> NilOrVal<NewValue> {
        match self {
            NilOrVal::Nil => NilOrVal::Nil,
            NilOrVal::Val(value) => NilOrVal::Val(f(value)),
        }
    }

    /// Convert this into an `NilOrVal<&Value>`, allowing to borrow the value.
    pub fn as_ref(&self) -> NilOrVal<&Value> {
        match self {
            NilOrVal::Nil => NilOrVal::Nil,
            NilOrVal::Val(value) => NilOrVal::Val(value),
        }
    }

    /// Consumes this and returns the value if it is not `nil`,
    /// otherwise returns the default `Value`.
    pub fn value_or_default(self) -> Value
    where
        Value: Default,
    {
        match self {
            NilOrVal::Nil => Value::default(),
            NilOrVal::Val(value) => value,
        }
    }
}

/// Defines the requirements for the type of value to decide on.
pub trait Value
where
    Self: Clone + Debug + PartialEq + Eq + PartialOrd + Ord + Send + Sync,
{
    /// The type of the ID of the value.
    /// Typically a representation of the value with a lower memory footprint.
    type Id: Clone + Debug + PartialEq + Eq + PartialOrd + Ord + Send + Sync;

    /// The ID of the value.
    fn id(&self) -> Self::Id;
}
