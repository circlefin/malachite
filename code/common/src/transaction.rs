use alloc::vec::Vec;
use core::fmt::Debug;

/// Transaction
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
}
