use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

use malachite_common::CommitCertificate;
use malachite_proto::Protobuf;

use prost::Message;
use redb::ReadableTable;
use thiserror::Error;

use crate::codec;
use crate::proto::{self as proto, Error as ProtoError};
use crate::types::MockContext;
use crate::types::{Block, Height, Transaction, Transactions};

#[derive(Clone, Debug)]
pub struct DecidedBlock {
    pub block: Block,
    pub certificate: CommitCertificate<MockContext>,
}

fn decode_certificate(bytes: &[u8]) -> Result<CommitCertificate<MockContext>, ProtoError> {
    let proto = proto::CommitCertificate::decode(bytes)?;
    codec::decode_certificate(proto)
}

fn encode_certificate(certificate: CommitCertificate<MockContext>) -> Result<Vec<u8>, ProtoError> {
    let proto = codec::encode_certificate(certificate)?;
    Ok(proto.encode_to_vec())
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

#[derive(Copy, Clone, Debug)]
struct HeightKey;

impl redb::Value for HeightKey {
    type SelfType<'a> = Height;

    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        Some(core::mem::size_of::<u64>() * 2)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let (fork_id, block_number) = <(u64, u64) as redb::Value>::from_bytes(data);

        Height {
            fork_id,
            block_number,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        <(u64, u64) as redb::Value>::as_bytes(&(value.fork_id, value.block_number))
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("starknet::Height")
    }
}

impl redb::Key for HeightKey {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        <(u64, u64) as redb::Key>::compare(data1, data2)
    }
}

const BLOCK_TABLE: redb::TableDefinition<HeightKey, Vec<u8>> = redb::TableDefinition::new("blocks");
const CERTIFICATE_TABLE: redb::TableDefinition<HeightKey, Vec<u8>> =
    redb::TableDefinition::new("certificates");

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
        let block = {
            let table = tx.open_table(BLOCK_TABLE)?;
            let value = table.get(&height)?;
            value.and_then(|value| Block::from_bytes(&value.value()).ok())
        };
        let certificate = {
            let table = tx.open_table(CERTIFICATE_TABLE)?;
            let value = table.get(&height)?;
            value.and_then(|value| decode_certificate(&value.value()).ok())
        };

        let decided_block = block
            .zip(certificate)
            .map(|(block, certificate)| DecidedBlock { block, certificate });

        Ok(decided_block)
    }

    fn insert(&self, decided_block: DecidedBlock) -> Result<(), StoreError> {
        let height = decided_block.block.height;

        let tx = self.db.begin_write()?;
        {
            let mut blocks = tx.open_table(BLOCK_TABLE)?;
            blocks.insert(height, decided_block.block.to_bytes()?.to_vec())?;
        }
        {
            let mut certificates = tx.open_table(CERTIFICATE_TABLE)?;
            certificates.insert(height, encode_certificate(decided_block.certificate)?)?;
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
            let mut blocks = tx.open_table(BLOCK_TABLE)?;
            let mut certificates = tx.open_table(CERTIFICATE_TABLE)?;
            let keys = self.range(&blocks, ..retain_height)?;
            for key in &keys {
                blocks.remove(key)?;
                certificates.remove(key)?;
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
        // Implicitly creates the tables if they do not exist yet
        let _ = tx.open_table(BLOCK_TABLE)?;
        let _ = tx.open_table(CERTIFICATE_TABLE)?;
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

    pub async fn store(
        &self,
        certificate: &CommitCertificate<MockContext>,
        txes: &[Transaction],
    ) -> Result<(), StoreError> {
        let decided_block = DecidedBlock {
            block: Block {
                height: certificate.height,
                block_hash: certificate.value_id,
                transactions: Transactions::new(txes.to_vec()),
            },
            certificate: certificate.clone(),
        };

        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.insert(decided_block)).await?
    }

    pub async fn prune(&self, retain_height: Height) -> Result<Vec<Height>, StoreError> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.prune(retain_height)).await?
    }
}
