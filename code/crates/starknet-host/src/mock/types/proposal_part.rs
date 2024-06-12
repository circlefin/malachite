use crate::mock::types::TransactionBatch;

pub enum ProposalPart {
    TxBatch(TransactionBatch),
    Proof(Vec<u8>),
}
