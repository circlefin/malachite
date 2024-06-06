use alloc::vec::Vec;
use core::fmt::Debug;

/// Transaction
/// TODO: Define this in the Context
#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Transaction(pub Vec<u8>);

impl Transaction {
    /// Create a new transaction from bytes
    pub const fn new(transaction: Vec<u8>) -> Self {
        Self(transaction)
    }

    /// Get bytes from a transaction
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    /// Size of this transaction in bytes
    pub fn size_bytes(&self) -> u64 {
        self.0.len() as u64
    }
}

/// Transaction batch (used by mempool and block part)
/// TODO: Parametrize by the Context
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionBatch(Vec<Transaction>);

impl TransactionBatch {
    /// Create a new transaction batch
    pub fn new(transactions: Vec<Transaction>) -> Self {
        TransactionBatch(transactions)
    }
    /// Get transactions from a batch
    pub fn transactions(&self) -> &Vec<Transaction> {
        &self.0
    }
}

/// Mempool transaction batch
// TODO: Move to different file
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MempoolTransactionBatch {
    // May add more fields to this structure
    transaction_batch: TransactionBatch,
}

impl MempoolTransactionBatch {
    /// Create a new transaction batch
    pub fn new(transaction_batch: TransactionBatch) -> Self {
        MempoolTransactionBatch { transaction_batch }
    }
    /// Get transactions from a batch
    pub fn transactions(&self) -> &TransactionBatch {
        &self.transaction_batch
    }
    /// Get the number of transactions in the batch
    pub fn len(&self) -> usize {
        self.transaction_batch.transactions().len()
    }
    /// Implement is_empty
    pub fn is_empty(&self) -> bool {
        self.transaction_batch.transactions().is_empty()
    }
}
