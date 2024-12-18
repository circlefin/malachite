use core::fmt;

use bytes::Bytes;
use malachitebft_proto::{self as proto};
use malachitebft_starknet_p2p_proto as p2p_proto;

use crate::Hash;

/// Transaction
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd)]
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
    pub fn hash(&self) -> Hash {
        self.hash
    }

    /// Compute the hash of a transaction
    ///
    /// TODO: Use hash function from Context
    pub fn compute_hash(bytes: &[u8]) -> Hash {
        use sha3::Digest;
        let mut hasher = sha3::Keccak256::new();
        hasher.update(bytes);
        Hash::new(hasher.finalize().into())
    }
}

impl fmt::Debug for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transaction({}, {} bytes)", self.hash, self.size_bytes())
    }
}

impl proto::Protobuf for Transaction {
    type Proto = p2p_proto::Transaction;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        use malachitebft_starknet_p2p_proto::transaction::Txn;

        let txn = proto
            .txn
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("txn"))?;

        let hash = proto
            .transaction_hash
            .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("transaction_hash"))?;

        match txn {
            Txn::Dummy(dummy) => Ok(Self {
                data: dummy.bytes,
                hash: Hash::from_proto(hash)?,
            }),
            _ => Err(proto::Error::invalid_data::<Self::Proto>(
                "unknown transaction type",
            )),
        }
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        use malachitebft_starknet_p2p_proto::transaction::{Dummy, Txn};

        Ok(Self::Proto {
            transaction_hash: Some(self.hash.to_proto()?),
            txn: Some(Txn::Dummy(Dummy {
                bytes: self.to_bytes(),
            })),
        })
    }
}

/// Transaction batch (used by mempool and proposal part)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transactions(Vec<Transaction>);

impl Transactions {
    /// Create a new transaction batch
    pub fn new(txes: Vec<Transaction>) -> Self {
        Transactions(txes)
    }

    /// Add a transaction to the batch
    pub fn push(&mut self, tx: Transaction) {
        self.0.push(tx);
    }

    /// Add a set of transaction to the batch
    pub fn append(&mut self, txes: Transactions) {
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

impl proto::Protobuf for Transactions {
    type Proto = p2p_proto::Transactions;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self::new(
            proto
                .transactions
                .into_iter()
                .map(Transaction::from_proto)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(p2p_proto::Transactions {
            transactions: self
                .as_slice()
                .iter()
                .map(Transaction::to_proto)
                .collect::<Result<_, _>>()?,
        })
    }
}
