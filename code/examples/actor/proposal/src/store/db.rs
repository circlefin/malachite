use std::ops::RangeBounds;
use std::path::Path;

use bytes::Bytes;
use redb::ReadableTable;
use tracing::error;

use malachitebft_codec::Codec;
use malachitebft_core_consensus::ProposedValue;
use malachitebft_core_types::Round;
use malachitebft_proto::Protobuf;

use crate::codec::ProtobufCodec;
use crate::types::{Block, BlockHash, Height, MockContext};

use super::error::StoreError;
use super::keys::{HeightKey, UndecidedValueKey};
use super::types::{decode_certificate, encode_certificate, DecidedBlock};

const CERTIFICATES_TABLE: redb::TableDefinition<HeightKey, Vec<u8>> =
    redb::TableDefinition::new("certificates");

const DECIDED_BLOCKS_TABLE: redb::TableDefinition<HeightKey, Vec<u8>> =
    redb::TableDefinition::new("decided_blocks");

const UNDECIDED_VALUES_TABLE: redb::TableDefinition<UndecidedValueKey, Vec<u8>> =
    redb::TableDefinition::new("undecided_blocks");

pub struct Db {
    db: redb::Database,
}

impl Db {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Ok(Self {
            db: redb::Database::create(path).map_err(StoreError::Database)?,
        })
    }

    pub fn get_decided_block(&self, height: Height) -> Result<Option<DecidedBlock>, StoreError> {
        let tx = self.db.begin_read()?;
        let block = {
            let table = tx.open_table(DECIDED_BLOCKS_TABLE)?;
            let value = table.get(&height)?;
            value.and_then(|value| Block::from_bytes(&value.value()).ok())
        };
        let certificate = {
            let table = tx.open_table(CERTIFICATES_TABLE)?;
            let value = table.get(&height)?;
            value.and_then(|value| decode_certificate(&value.value()).ok())
        };

        let decided_block = block
            .zip(certificate)
            .map(|(block, certificate)| DecidedBlock { block, certificate });

        Ok(decided_block)
    }

    pub fn insert_decided_block(&self, decided_block: DecidedBlock) -> Result<(), StoreError> {
        let height = decided_block.block.height;

        let tx = self.db.begin_write()?;
        {
            let mut blocks = tx.open_table(DECIDED_BLOCKS_TABLE)?;
            blocks.insert(height, decided_block.block.to_bytes()?.to_vec())?;
        }
        {
            let mut certificates = tx.open_table(CERTIFICATES_TABLE)?;
            certificates.insert(height, encode_certificate(&decided_block.certificate)?)?;
        }
        tx.commit()?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn get_undecided_values(
        &self,
        height: Height,
        round: Round,
    ) -> Result<Vec<ProposedValue<MockContext>>, StoreError> {
        let tx = self.db.begin_read()?;
        let mut values = Vec::new();

        let from = (height, round, BlockHash::new([0; 32]));
        let to = (height, round, BlockHash::new([255; 32]));

        let table = tx.open_table(UNDECIDED_VALUES_TABLE)?;
        let keys = self.undecided_values_range(&table, from..to)?;

        for key in keys {
            if let Ok(Some(value)) = table.get(&key) {
                let Ok(value) = ProtobufCodec.decode(Bytes::from(value.value())) else {
                    error!(hash = ?key.2, "Failed to decode ProposedValue");
                    continue;
                };

                values.push(value);
            }
        }

        Ok(values)
    }

    pub fn insert_undecided_value(
        &self,
        value: ProposedValue<MockContext>,
    ) -> Result<(), StoreError> {
        let key = (
            value.height,
            value.round,
            value.value.id().as_hash().clone(),
        );
        let value = ProtobufCodec.encode(&value)?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(UNDECIDED_VALUES_TABLE)?;
            table.insert(key, value.to_vec())?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn height_range<Table>(
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

    pub fn undecided_values_range<Table>(
        &self,
        table: &Table,
        range: impl RangeBounds<(Height, Round, BlockHash)>,
    ) -> Result<Vec<(Height, Round, BlockHash)>, StoreError>
    where
        Table: redb::ReadableTable<UndecidedValueKey, Vec<u8>>,
    {
        Ok(table
            .range(range)?
            .flatten()
            .map(|(key, _)| key.value())
            .collect::<Vec<_>>())
    }

    pub fn prune(&self, retain_height: Height) -> Result<Vec<Height>, StoreError> {
        let tx = self.db.begin_write().unwrap();
        let pruned = {
            let mut undecided = tx.open_table(UNDECIDED_VALUES_TABLE)?;
            let keys = self.undecided_values_range(
                &undecided,
                ..(retain_height, Round::Nil, BlockHash::new([0; 32])),
            )?;
            for key in keys {
                undecided.remove(key)?;
            }

            let mut decided = tx.open_table(DECIDED_BLOCKS_TABLE)?;
            let mut certificates = tx.open_table(CERTIFICATES_TABLE)?;

            let keys = self.height_range(&decided, ..retain_height)?;
            for key in &keys {
                decided.remove(key)?;
                certificates.remove(key)?;
            }
            keys
        };
        tx.commit()?;

        Ok(pruned)
    }

    pub fn first_key(&self) -> Option<Height> {
        let tx = self.db.begin_read().unwrap();
        let table = tx.open_table(DECIDED_BLOCKS_TABLE).unwrap();
        let (key, _) = table.first().ok()??;
        Some(key.value())
    }

    pub fn last_key(&self) -> Option<Height> {
        let tx = self.db.begin_read().unwrap();
        let table = tx.open_table(DECIDED_BLOCKS_TABLE).unwrap();
        let (key, _) = table.last().ok()??;
        Some(key.value())
    }

    pub fn create_tables(&self) -> Result<(), StoreError> {
        let tx = self.db.begin_write()?;
        // Implicitly creates the tables if they do not exist yet
        let _ = tx.open_table(DECIDED_BLOCKS_TABLE)?;
        let _ = tx.open_table(CERTIFICATES_TABLE)?;
        let _ = tx.open_table(UNDECIDED_VALUES_TABLE)?;
        tx.commit()?;
        Ok(())
    }
}
