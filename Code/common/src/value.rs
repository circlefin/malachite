use core::fmt::Debug;

/// Represents either `Nil` or a value of type `Value`.
///
/// This type is isomorphic to `Option<Value>` but is more explicit about its intent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum NilOrVal<Value> {
    #[default]
    Nil,

    Val(Value),
}

impl<Value> NilOrVal<Value> {
    pub fn is_nil(&self) -> bool {
        matches!(self, Self::Nil)
    }

    pub fn is_val(&self) -> bool {
        matches!(self, Self::Val(_))
    }

    pub fn map<NewValue, F: FnOnce(Value) -> NewValue>(self, f: F) -> NilOrVal<NewValue> {
        match self {
            NilOrVal::Nil => NilOrVal::Nil,
            NilOrVal::Val(value) => NilOrVal::Val(f(value)),
        }
    }

    pub fn as_ref(&self) -> NilOrVal<&Value> {
        match self {
            NilOrVal::Nil => NilOrVal::Nil,
            NilOrVal::Val(value) => NilOrVal::Val(value),
        }
    }

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
    Self: Clone + Debug + PartialEq + Eq + PartialOrd + Ord,
{
    /// The type of the ID of the value.
    /// Typically a representation of the value with a lower memory footprint.
    type Id: Clone + Debug + PartialEq + Eq + PartialOrd + Ord;

    /// The ID of the value.
    fn id(&self) -> Self::Id;
}
