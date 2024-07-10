use core::fmt::Debug;

/// Mempool transaction batch
#[derive(Clone, Debug, PartialEq)]
pub struct MempoolTransactionBatch {
    /// The batch of transactions
    pub transaction_batch: prost_types::Any,
    // May add more fields to this structure
}

impl MempoolTransactionBatch {
    /// Create a new transaction batch
    pub fn new(transaction_batch: prost_types::Any) -> Self {
        Self { transaction_batch }
    }
}
