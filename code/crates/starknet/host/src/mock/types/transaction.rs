/// Transaction
#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Transaction(Vec<u8>);

impl Transaction {
    /// Create a new transaction from bytes
    pub const fn new(transaction: Vec<u8>) -> Self {
        Self(transaction)
    }

    /// Get bytes from a transaction
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    /// Get bytes from a transaction
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Size of this transaction in bytes
    pub fn size_bytes(&self) -> usize {
        self.0.len()
    }
}

/// Transaction batch (used by mempool and block part)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransactionBatch(Vec<Transaction>);

impl TransactionBatch {
    /// Create a new transaction batch
    pub fn new(transactions: Vec<Transaction>) -> Self {
        TransactionBatch(transactions)
    }

    /// Add a transaction to the batch
    pub fn push(&mut self, transaction: Transaction) {
        self.0.push(transaction);
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
    pub fn into_transactions(self) -> Vec<Transaction> {
        self.0
    }

    /// Get transactions from a batch
    pub fn transactions(&self) -> &[Transaction] {
        &self.0
    }

    /// The size of this batch in bytes
    pub fn size_bytes(&self) -> usize {
        self.transactions()
            .iter()
            .map(|tx| tx.size_bytes())
            .sum::<usize>()
    }
}
