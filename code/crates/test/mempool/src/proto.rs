//! Protobuf instances for mempool types

#![allow(missing_docs)]

pub use malachitebft_proto::{Error, Protobuf};

include!(concat!(env!("OUT_DIR"), "/malachite.mempool.rs"));

impl Protobuf for crate::types::MempoolTransactionBatch {
    type Proto = MempoolTransactionBatch;

    fn from_proto(proto: Self::Proto) -> Result<Self, Error> {
        Ok(Self::new(proto.transaction_batch.ok_or_else(|| {
            Error::missing_field::<Self::Proto>("content")
        })?))
    }

    fn to_proto(&self) -> Result<Self::Proto, Error> {
        Ok(MempoolTransactionBatch {
            transaction_batch: Some(self.transaction_batch.clone()),
        })
    }
}
