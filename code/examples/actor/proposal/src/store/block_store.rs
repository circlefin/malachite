use std::{path::Path, sync::Arc};

use malachitebft_core_consensus::ProposedValue;
use malachitebft_core_types::{CommitCertificate, Round};

use crate::types::{Block, Height, MockContext};

use super::{db::Db, error::StoreError, types::DecidedBlock};

pub struct BlockStore {
    db: Arc<Db>,
}

impl Clone for BlockStore {
    fn clone(&self) -> Self {
        BlockStore {
            db: Arc::clone(&self.db),
        }
    }
}

impl BlockStore {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_owned();
        tokio::task::spawn_blocking(move || {
            let db = Db::new(path)?;
            db.create_tables()?;
            Ok(Self { db: Arc::new(db) })
        })
        .await?
    }

    pub async fn min_decided_value_height(&self) -> Option<Height> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.first_key())
            .await
            .ok()
            .flatten()
    }

    pub async fn max_decided_value_height(&self) -> Option<Height> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.last_key())
            .await
            .ok()
            .flatten()
    }

    pub async fn get_decided_value(
        &self,
        height: Height,
    ) -> Result<Option<DecidedBlock>, StoreError> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.get_decided_block(height)).await?
    }

    pub async fn store_decided_block(
        &self,
        certificate: &CommitCertificate<MockContext>,
        block: &Block,
    ) -> Result<(), StoreError> {
        let decided_block = DecidedBlock {
            block: block.clone(),
            certificate: certificate.clone(),
        };
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.insert_decided_block(decided_block)).await?
    }

    pub async fn store_undecided_proposal(
        &self,
        value: ProposedValue<MockContext>,
    ) -> Result<(), StoreError> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.insert_undecided_value(value)).await?
    }

    pub async fn get_undecided_proposals(
        &self,
        height: Height,
        round: Round,
    ) -> Result<Vec<ProposedValue<MockContext>>, StoreError> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.get_undecided_values(height, round)).await?
    }

    pub async fn prune(&self, retain_height: Height) -> Result<Vec<Height>, StoreError> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.prune(retain_height)).await?
    }
}
