pub mod mock {
    include!(concat!(env!("OUT_DIR"), "/starknet.mock.rs"));
}

pub use malachite_proto::{Error, Protobuf};

use crate::mock::types;

impl Protobuf for types::Transaction {
    type Proto = mock::Transaction;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        let tx = proto
            .value
            .ok_or_else(|| Error::Other("Missing field `value`".to_string()))?;

        Ok(Self::new(tx))
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        let value = self.to_bytes();
        Ok(Self::Proto { value: Some(value) })
    }
}

impl Protobuf for types::TransactionBatch {
    type Proto = mock::TransactionBatch;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        Ok(Self::new(
            proto
                .transactions
                .into_iter()
                .map(types::Transaction::from_proto)
                .collect::<Result<_, _>>()?,
        ))
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        Ok(Self::Proto {
            transactions: self
                .transactions()
                .iter()
                .map(|t| t.to_proto())
                .collect::<Result<_, _>>()?,
        })
    }
}
