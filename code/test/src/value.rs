use malachite_proto::{self as proto};

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

impl proto::Protobuf for ValueId {
    type Proto = proto::ValueId;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let bytes = proto
            .value
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?;

        let bytes = <[u8; 8]>::try_from(bytes)
            .map_err(|_| proto::Error::Other("Invalid value length".to_string()))?;

        Ok(ValueId::new(u64::from_be_bytes(bytes)))
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::ValueId {
            value: Some(self.0.to_be_bytes().to_vec()),
        })
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

impl proto::Protobuf for Value {
    type Proto = proto::Value;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let bytes = proto
            .value
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?;

        let bytes = <[u8; 8]>::try_from(bytes)
            .map_err(|_| proto::Error::Other("Invalid value length".to_string()))?;

        Ok(Value::new(u64::from_be_bytes(bytes)))
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::Value {
            value: Some(self.0.to_be_bytes().to_vec()),
        })
    }
}
