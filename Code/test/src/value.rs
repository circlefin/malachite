#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub struct ValueId(u64);

impl ValueId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

/// The value to decide on
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Value(u64);

impl Value {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    pub const fn valid(&self) -> bool {
        self.0 > 0
    }

    pub const fn id(&self) -> ValueId {
        ValueId(self.0)
    }
}

impl malachite_common::Value for Value {
    type Id = ValueId;

    fn is_valid(&self) -> bool {
        self.valid()
    }

    fn id(&self) -> ValueId {
        self.id()
    }
}
