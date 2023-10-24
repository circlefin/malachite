/// The value to decide on
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Value(u64);

impl Value {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn valid(&self) -> bool {
        self.0 > 0
    }

    pub fn id(&self) -> ValueId {
        ValueId(self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub struct ValueId(u64);

impl ValueId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}
