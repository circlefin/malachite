pub use malachite_common::Transaction;
pub use malachite_common::TransactionBatch;

// use core::fmt::Debug;
//
// use malachite_proto as proto;
//
// /// Transaction
// #[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
// pub struct Transaction(Vec<u8>);
//
// impl Transaction {
//     /// Create a new transaction from bytes
//     pub const fn new(bytes: Vec<u8>) -> Self {
//         Self(bytes)
//     }
//
//     /// Get bytes from a transaction
//     pub fn as_bytes(&self) -> &[u8] {
//         &self.0
//     }
//
//     /// Get bytes from a transaction
//     pub fn to_bytes(&self) -> Vec<u8> {
//         self.0.to_vec()
//     }
//
//     /// Get bytes from a transaction
//     pub fn into_bytes(self) -> Vec<u8> {
//         self.0
//     }
//
//     /// Size of this transaction in bytes
//     pub fn size_bytes(&self) -> usize {
//         self.0.len()
//     }
// }
//
// impl proto::Protobuf for Transaction {
//     type Proto = crate::proto::mock::Transaction;
//
//     fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
//         Ok(Self::new(proto.value))
//     }
//
//     fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
//         Ok(crate::proto::mock::Transaction {
//             value: self.to_bytes(),
//         })
//     }
// }
//
// /// Transaction batch (used by mempool and block part)
// #[derive(Clone, Debug, Default, PartialEq, Eq)]
// pub struct TransactionBatch(Vec<Transaction>);
//
// impl TransactionBatch {
//     /// Create a new transaction batch
//     pub fn new(transactions: Vec<Transaction>) -> Self {
//         TransactionBatch(transactions)
//     }
//
//     /// Add a transaction to the batch
//     pub fn push(&mut self, transaction: Transaction) {
//         self.0.push(transaction);
//     }
//
//     /// Get the number of transactions in the batch
//     pub fn len(&self) -> usize {
//         self.0.len()
//     }
//
//     /// Whether or not the batch is empty
//     pub fn is_empty(&self) -> bool {
//         self.0.is_empty()
//     }
//
//     /// Get transactions from a batch
//     pub fn into_transactions(self) -> Vec<Transaction> {
//         self.0
//     }
//
//     /// Get transactions from a batch
//     pub fn transactions(&self) -> &[Transaction] {
//         &self.0
//     }
//
//     /// The size of this batch in bytes
//     pub fn size_bytes(&self) -> usize {
//         self.transactions()
//             .iter()
//             .map(|tx| tx.size_bytes())
//             .sum::<usize>()
//     }
// }
//
// impl proto::Protobuf for TransactionBatch {
//     type Proto = crate::proto::mock::TransactionBatch;
//
//     fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
//         Ok(Self::new(
//             proto
//                 .transactions
//                 .into_iter()
//                 .map(Transaction::from_proto)
//                 .collect::<Result<_, _>>()?,
//         ))
//     }
//
//     fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
//         Ok(crate::proto::mock::TransactionBatch {
//             transactions: self
//                 .transactions()
//                 .iter()
//                 .map(Transaction::to_proto)
//                 .collect::<Result<_, _>>()?,
//         })
//     }
// }
//
// /// Mempool transaction batch
// #[derive(Clone, Debug, PartialEq, Eq)]
// pub struct MempoolTransactionBatch {
//     /// The batch of transactions
//     pub tx_batch: TransactionBatch,
//     // May add more fields to this structure
// }
//
// impl MempoolTransactionBatch {
//     /// Create a new transaction batch
//     pub fn new(tx_batch: TransactionBatch) -> Self {
//         Self { tx_batch }
//     }
//
//     /// Get the number of transactions in the batch
//     pub fn len(&self) -> usize {
//         self.tx_batch.len()
//     }
//
//     /// Implement is_empty
//     pub fn is_empty(&self) -> bool {
//         self.tx_batch.is_empty()
//     }
// }
//
// impl proto::Protobuf for MempoolTransactionBatch {
//     type Proto = crate::proto::mock::MempoolTransactionBatch;
//
//     fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
//         let tx_batch = proto
//             .tx_batch
//             .ok_or(proto::Error::missing_field::<Self::Proto>("tx_batch"))?;
//
//         Ok(Self {
//             tx_batch: TransactionBatch::from_proto(tx_batch)?,
//         })
//     }
//
//     fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
//         Ok(crate::proto::mock::MempoolTransactionBatch {
//             tx_batch: Some(self.tx_batch.to_proto()?),
//         })
//     }
// }
