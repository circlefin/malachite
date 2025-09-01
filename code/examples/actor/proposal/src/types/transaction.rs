use core::fmt;

use crate::types::hash::Hash;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use malachitebft_proto::{Error as ProtoError, Protobuf};

use super::proto;

/// Transaction
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Transaction {
    data: Bytes,
    hash: Hash,
}

impl Transaction {
    /// Create a new transaction from bytes
    pub fn new(data: impl Into<Bytes>) -> Self {
        let data = data.into();
        let hash = Self::compute_hash(&data);
        Self { data, hash }
    }

    /// Get bytes from a transaction
    pub fn to_bytes(&self) -> Bytes {
        self.data.clone()
    }

    /// Get bytes from a transaction
    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_ref()
    }

    /// Size of this transaction in bytes
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }

    /// Hash of this transaction
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Compute the hash of a transaction
    pub fn compute_hash(bytes: &[u8]) -> Hash {
        use sha3::Digest;
        let mut hasher = sha3::Keccak256::new();
        hasher.update(bytes);
        Hash::new(hasher.finalize().into())
    }
}

impl fmt::Debug for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Transaction({:?}, {} bytes)",
            self.hash,
            self.size_bytes()
        )
    }
}

impl Protobuf for Transaction {
    type Proto = proto::ConsensusTransaction;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        let data = proto.tx;
        let hash = Self::compute_hash(&data);
        Ok(Self { data, hash })
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            tx: self.data.clone(),
            hash: Bytes::from(self.hash.to_vec()),
        })
    }
}

/// Transaction batch (used by mempool)
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TransactionBatch(Vec<Transaction>);

impl TransactionBatch {
    /// Create a new transaction batch
    pub fn new(txes: Vec<Transaction>) -> Self {
        TransactionBatch(txes)
    }

    /// Add a transaction to the batch
    pub fn push(&mut self, tx: Transaction) {
        self.0.push(tx);
    }

    /// Add a set of transaction to the batch
    pub fn append(&mut self, txes: TransactionBatch) {
        let mut txes1 = txes.clone();
        self.0.append(&mut txes1.0);
    }

    /// Get the number of transactions in the batch
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether or not the batch is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get transactions from a batch
    pub fn into_vec(self) -> Vec<Transaction> {
        self.0
    }

    /// Get transactions from a batch
    pub fn to_vec(&self) -> Vec<Transaction> {
        self.0.to_vec()
    }

    /// Get transactions from a batch
    pub fn as_slice(&self) -> &[Transaction] {
        &self.0
    }

    /// The size of this batch in bytes
    pub fn size_bytes(&self) -> usize {
        self.as_slice()
            .iter()
            .map(|tx| tx.size_bytes())
            .sum::<usize>()
    }
}

impl Protobuf for TransactionBatch {
    type Proto = proto::TransactionBatch;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self::new(
            proto
                .transactions
                .into_iter()
                .map(Transaction::from_proto)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(proto::TransactionBatch {
            transactions: self
                .as_slice()
                .iter()
                .map(Transaction::to_proto)
                .collect::<Result<_, _>>()?,
        })
    }
}
