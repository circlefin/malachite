#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub struct ValueId(u64);

impl ValueId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<u64> for ValueId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl TryFrom<malachite_proto::ValueId> for ValueId {
    type Error = String;

    fn try_from(proto: malachite_proto::ValueId) -> Result<Self, Self::Error> {
        match proto.value {
            Some(bytes) => {
                let bytes = <[u8; 8]>::try_from(bytes).unwrap(); // FIXME
                Ok(ValueId::new(u64::from_be_bytes(bytes)))
            }
            None => Err("ValueId not present".to_string()),
        }
    }
}

impl From<ValueId> for malachite_proto::ValueId {
    fn from(value: ValueId) -> malachite_proto::ValueId {
        malachite_proto::ValueId {
            value: Some(value.0.to_be_bytes().to_vec()),
        }
    }
}

/// The value to decide on
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Value(u64);

impl Value {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    pub const fn id(&self) -> ValueId {
        ValueId(self.0)
    }
}

impl malachite_common::Value for Value {
    type Id = ValueId;

    fn id(&self) -> ValueId {
        self.id()
    }
}

impl TryFrom<malachite_proto::Value> for Value {
    type Error = String;

    fn try_from(proto: malachite_proto::Value) -> Result<Self, Self::Error> {
        match proto.value {
            Some(bytes) => {
                let bytes = <[u8; 8]>::try_from(bytes).unwrap(); // FIXME
                let value = u64::from_be_bytes(bytes);
                Ok(Value::new(value))
            }
            None => Err("Value not present".to_string()),
        }
    }
}

impl From<Value> for malachite_proto::Value {
    fn from(value: Value) -> malachite_proto::Value {
        malachite_proto::Value {
            value: Some(value.0.to_be_bytes().to_vec()),
        }
    }
}
