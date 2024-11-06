use std::path::Path;
use std::sync::Arc;

use prost::Message;
use redb::ReadableTable;

use malachite_blocksync::SyncedBlock;
use malachite_common::Value;
use malachite_common::{Certificate, Proposal, SignedProposal, SignedVote};

use crate::codec::{decode_sync_block, encode_synced_block};
use crate::mock::context::MockContext;
use crate::proto::{self as proto, Protobuf};
use crate::types::{Block, Height, Transaction, Transactions};

#[derive(Clone, Debug)]
pub struct DecidedBlock {
    pub block: Block,
    pub proposal: SignedProposal<MockContext>,
    pub certificate: Certificate<MockContext>,
}

impl DecidedBlock {
    fn to_bytes(&self) -> Vec<u8> {
        let synced_block = SyncedBlock {
            block_bytes: self.block.to_bytes().unwrap(),
            proposal: self.proposal.clone(),
            certificate: self.certificate.clone(),
        };

        let proto = encode_synced_block(synced_block).unwrap();
        proto.encode_to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let synced_block = proto::blocksync::SyncedBlock::decode(bytes).ok()?;
        let synced_block = decode_sync_block(synced_block).ok()?;
        let block = Block::from_bytes(synced_block.block_bytes.as_ref()).ok()?;

        Some(Self {
            block,
            proposal: synced_block.proposal,
            certificate: synced_block.certificate,
        })
    }
}

#[derive(Debug)]
pub enum StoreError {
    Database(redb::DatabaseError),
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

struct Db {
    db: redb::Database,
}

impl Db {
    fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Ok(Self {
            db: redb::Database::create(path).map_err(StoreError::Database)?,
        })
    }

    fn keys(&self) -> Vec<Height> {
        let tx = self.db.begin_read().unwrap();
        let table = tx.open_table(BLOCK_TABLE).unwrap();
        table
            .iter()
            .unwrap()
            .filter_map(|result| result.ok())
            .map(|(key, _)| key.value())
            .collect()
    }

    fn get(&self, height: Height) -> Option<DecidedBlock> {
        let tx = self.db.begin_read().unwrap();
        let table = tx.open_table(BLOCK_TABLE).unwrap();
        let value = table.get(&height).unwrap()?;
        DecidedBlock::from_bytes(&value.value())
    }

    fn insert(&self, decided_block: DecidedBlock) {
        let height = decided_block.block.height;

        let tx = self.db.begin_write().unwrap();
        {
            let mut table = tx.open_table(BLOCK_TABLE).unwrap();
            table.insert(height, decided_block.to_bytes()).unwrap();
        }
        tx.commit().unwrap();
    }

    fn prune(&self, retain_height: Height) {
        let tx = self.db.begin_write().unwrap();
        {
            let mut table = tx.open_table(BLOCK_TABLE).unwrap();
            table.retain(|key, _| key > retain_height).unwrap();
        }
        tx.commit().unwrap();
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
}

#[derive(Clone)]
pub struct BlockStore {
    db: Arc<Db>,
}

impl BlockStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Ok(Self {
            db: Arc::new(Db::new(path)?),
        })
    }

    pub fn first_height(&self) -> Option<Height> {
        self.db.first_key()
    }

    pub fn last_height(&self) -> Option<Height> {
        self.db.last_key()
    }

    pub fn keys(&self) -> Vec<Height> {
        self.db.keys()
    }

    pub async fn get(&self, height: Height) -> Option<DecidedBlock> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || db.get(height))
            .await
            .unwrap()
    }

    pub async fn store(
        &self,
        proposal: &SignedProposal<MockContext>,
        txes: &[Transaction],
        commits: &[SignedVote<MockContext>],
    ) {
        let block_id = proposal.value().id();

        let certificate = Certificate {
            commits: commits.to_vec(),
        };

        let decided_block = DecidedBlock {
            block: Block {
                height: proposal.height(),
                block_hash: block_id,
                transactions: Transactions::new(txes.to_vec()),
            },
            proposal: proposal.clone(),
            certificate,
        };

        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.insert(decided_block);
        })
        .await
        .unwrap();
    }

    pub async fn prune(&self, retain_height: Height) {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.prune(retain_height);
        })
        .await
        .unwrap();
    }
}
