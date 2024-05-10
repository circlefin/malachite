use malachite_common::Transaction;
use malachite_proto::{self as proto};
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub struct ValueId(u64);

impl ValueId {
    pub fn new_from_value(value: Value) -> Self {
        let mut hash = DefaultHasher::new();
        let txs = value.0;
        txs.hash(&mut hash);
        ValueId(hash.finish())
    }
    pub fn new_from_u64(id: u64) -> Self {
        ValueId(id)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<Value> for ValueId {
    fn from(block: Value) -> Self {
        Self::new_from_value(block)
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

        Ok(ValueId(u64::from_be_bytes(bytes)))
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::ValueId {
            value: Some(self.0.to_be_bytes().to_vec()),
        })
    }
}

/// The value to decide on
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Value(Vec<Transaction>);

impl Value {
    pub fn new(txes: Vec<Transaction>) -> Self {
        Self(txes)
    }
    pub fn id(&self) -> ValueId {
        let mut hash = DefaultHasher::new();
        let txs = &self.0;
        txs.hash(&mut hash);
        ValueId(hash.finish())
    }
    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
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
        let mut txes = vec![];
        for raw_tx in proto.value.iter() {
            txes.push(Transaction::new(raw_tx.to_vec()));
        }

        Ok(Value::new(txes))
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        let mut raw_txes = vec![];
        for tx in self.0.iter() {
            raw_txes.push(tx.to_bytes());
        }
        Ok(proto::Value { value: raw_txes })
    }
}
