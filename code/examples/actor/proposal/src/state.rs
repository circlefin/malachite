use malachitebft_core_consensus::Role;
use std::path::Path;

use crate::types::{Address, Ed25519Provider, Height, MockContext};
use rand::RngCore;

use malachitebft_core_types::Round;
use malachitebft_engine::consensus::ConsensusRef;

use crate::app::App;
use crate::store::BlockStore;

pub struct HostState {
    pub ctx: MockContext,
    pub signing_provider: Ed25519Provider,
    pub height: Height,
    pub round: Round,
    pub proposer: Option<Address>,
    pub role: Role,
    pub app: App,
    pub consensus: Option<ConsensusRef<MockContext>>,
    pub block_store: BlockStore,
    pub nonce: u64,
}

impl HostState {
    pub async fn new<R>(
        ctx: MockContext,
        signing_provider: Ed25519Provider,
        app: App,
        db_path: impl AsRef<Path>,
        rng: &mut R,
    ) -> Self
    where
        R: RngCore,
    {
        Self {
            ctx,
            signing_provider,
            height: Height::new(1),
            round: Round::Nil,
            proposer: None,
            role: Role::None,
            app,
            consensus: None,
            block_store: BlockStore::new(db_path).await.unwrap(),
            nonce: rng.next_u64(),
        }
    }
}
