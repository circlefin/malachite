use malachite_common::MempoolTransactionBatch;
use malachite_proto::{Error as ProtoError, Protobuf};

use crate::Channel;

#[derive(Clone, Debug, PartialEq)]
pub enum NetworkMsg {
    TransactionBatch(MempoolTransactionBatch),
}

impl NetworkMsg {
    pub fn channel(&self) -> Channel {
        Channel::Mempool
    }

    pub fn from_network_bytes(bytes: &[u8]) -> Result<Self, ProtoError> {
        Protobuf::from_bytes(bytes).map(NetworkMsg::TransactionBatch)
    }

    pub fn to_network_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        match self {
            NetworkMsg::TransactionBatch(batch) => batch.to_bytes(),
        }
    }
}
