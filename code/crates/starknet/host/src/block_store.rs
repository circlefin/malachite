use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

use prost::Message;
use redb::ReadableTable;
use thiserror::Error;

use malachite_blocksync::SyncedBlock;
use malachite_common::CommitCertificate;
use malachite_consensus::ProposedValue;
use malachite_proto::Protobuf;

use crate::codec::{decode_sync_block, encode_proposed_value, encode_synced_block};
use crate::proto::{self as proto, Error as ProtoError};
use crate::types::MockContext;
use crate::types::{Block, Height, Transaction, Transactions};

mod keys;
use keys::{HeightKey, ProposedValueKey};

#[derive(Clone, Debug)]
pub struct DecidedBlock {
    pub block: Block,
    pub certificate: CommitCertificate<MockContext>,
}

impl DecidedBlock {
    fn into_bytes(self) -> Result<Vec<u8>, ProtoError> {
        let synced_block = SyncedBlock {
            certificate: self.certificate.clone(),
            block_bytes: self.block.to_bytes().unwrap(),
        };

        let proto = encode_synced_block(synced_block)?;
        Ok(proto.encode_to_vec())
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let synced_block = proto::blocksync::SyncedBlock::decode(bytes).ok()?;
        let synced_block = decode_sync_block(synced_block).ok()?;
        let block = Block::from_bytes(synced_block.block_bytes.as_ref()).ok()?;

        Some(Self {
            block,
            certificate: synced_block.certificate,
        })
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Database(#[from] redb::DatabaseError),

    #[error("Storage error: {0}")]
    Storage(#[from] redb::StorageError),

    #[error("Table error: {0}")]
    Table(#[from] redb::TableError),

    #[error("Commit error: {0}")]
    Commit(#[from] redb::CommitError),

    #[error("Transaction error: {0}")]
    Transaction(#[from] redb::TransactionError),

    #[error("Failed to encode/decode Protobuf: {0}")]
    Protobuf(#[from] ProtoError),

    #[error("Failed to join on task: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),
}

const BLOCK_TABLE: redb::TableDefinition<HeightKey, Vec<u8>> = redb::TableDefinition::new("blocks");

const PROPOSED_VALUE_TABLE: redb::TableDefinition<ProposedValueKey, Vec<u8>> =
    redb::TableDefinition::new("proposed_values");

struct Db {
    db: redb::Database,
}

impl Db {
    fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Ok(Self {
            db: redb::Database::create(path).map_err(StoreError::Database)?,
        })
    }

    fn get(&self, height: Height) -> Result<Option<DecidedBlock>, StoreError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(BLOCK_TABLE)?;
        let value = table.get(&height)?;
        let block = value.and_then(|value| DecidedBlock::from_bytes(&value.value()));
        Ok(block)
    }

    fn insert_decided_block(&self, decided_block: DecidedBlock) -> Result<(), StoreError> {
        let height = decided_block.block.height;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(BLOCK_TABLE)?;
            table.insert(height, decided_block.into_bytes()?)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn insert_proposed_value(&self, value: ProposedValue<MockContext>) -> Result<(), StoreError> {
        let key = (value.height, value.round, value.value);
        let value = encode_proposed_value(&value)?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(PROPOSED_VALUE_TABLE)?;
            table.insert(key, value)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn range<Table>(
        &self,
        table: &Table,
        range: impl RangeBounds<Height>,
    ) -> Result<Vec<Height>, StoreError>
    where
        Table: redb::ReadableTable<HeightKey, Vec<u8>>,
    {
        Ok(table
            .range(range)?
            .flatten()
            .map(|(key, _)| key.value())
            .collect::<Vec<_>>())
    }

    fn prune(&self, retain_height: Height) -> Result<Vec<Height>, StoreError> {
        let tx = self.db.begin_write().unwrap();
        let pruned = {
            let mut table = tx.open_table(BLOCK_TABLE)?;
            let keys = self.range(&table, ..retain_height)?;
            for key in &keys {
                table.remove(key)?;
            }
            keys
        };
        tx.commit()?;

        Ok(pruned)
    }

    fn first_key(&self) -> Option<Height> {
        let tx = self.db.begin_read().unwrap();
        let table = tx.open_table(BLOCK_TABLE).unwrap();
        let (key, _) = table.first().ok()??;
        Some(key.value())
    }

    fn last_key(&self) -> Option<Height> {
        let tx = self.db.begin_read().unwrap();
        let table = tx.open_table(BLOCK_TABLE).unwrap();
        let (key, _) = table.last().ok()??;
        Some(key.value())
    }

    fn create_tables(&self) -> Result<(), StoreError> {
        let tx = self.db.begin_write()?;
        // `open_table` implicitly creates the tables if needed
        let _ = tx.open_table(BLOCK_TABLE)?;
        let _ = tx.open_table(PROPOSED_VALUE_TABLE)?;
        tx.commit()?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct BlockStore {
    db: Arc<Db>,
}

impl BlockStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let db = Db::new(path)?;
        db.create_tables()?;

        Ok(Self { db: Arc::new(db) })
    }

    pub fn first_height(&self) -> Option<Height> {
        self.db.first_key()
    }

    pub fn last_height(&self) -> Option<Height> {
        self.db.last_key()
    }

    pub async fn get(&self, height: Height) -> Result<Option<DecidedBlock>, StoreError> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.get(height)).await?
    }

    pub async fn store_decided_block(
        &self,
        certificate: &CommitCertificate<MockContext>,
        txes: &[Transaction],
    ) -> Result<(), StoreError> {
        let block_id = certificate.value_id;

        let decided_block = DecidedBlock {
            block: Block {
                height: certificate.height,
                block_hash: block_id,
                transactions: Transactions::new(txes.to_vec()),
            },
            certificate: certificate.clone(),
        };

        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.insert_decided_block(decided_block)).await?
    }

    pub async fn store_proposed_value(
        &self,
        value: ProposedValue<MockContext>,
    ) -> Result<(), StoreError> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.insert_proposed_value(value)).await?
    }

    pub async fn prune(&self, retain_height: Height) -> Result<Vec<Height>, StoreError> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.prune(retain_height)).await?
    }
}
