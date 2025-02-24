use std::time::Duration;

use bytesize::ByteSize;
use malachitebft_config::{Config, PubSubProtocol, ValuePayload, VoteSyncMode};

#[derive(Copy, Clone, Debug)]
pub struct TestParams {
    pub enable_value_sync: bool,
    pub protocol: PubSubProtocol,
    pub block_size: ByteSize,
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub vote_extensions: Option<ByteSize>,
    pub value_payload: ValuePayload,
    pub max_retain_blocks: usize,
    pub timeout_step: Duration,
    pub vote_sync_mode: Option<VoteSyncMode>,
}

impl Default for TestParams {
    fn default() -> Self {
        Self {
            enable_value_sync: false,
            protocol: PubSubProtocol::default(),
            block_size: ByteSize::mib(1),
            tx_size: ByteSize::kib(1),
            txs_per_part: 256,
            vote_extensions: None,
            value_payload: ValuePayload::PartsOnly,
            max_retain_blocks: 50,
            timeout_step: Duration::from_secs(30),
            vote_sync_mode: None,
        }
    }
}

impl TestParams {
    pub fn apply_to_config(&self, config: &mut Config) {
        config.value_sync.enabled = self.enable_value_sync;
        config.consensus.p2p.protocol = self.protocol;
        config.consensus.timeouts.timeout_step = self.timeout_step;
        config.test.value_payload = self.value_payload;
        config.test.max_block_size = self.block_size;
        config.test.tx_size = self.tx_size;
        config.test.txs_per_part = self.txs_per_part;
        config.test.vote_extensions.enabled = self.vote_extensions.is_some();
        config.test.vote_extensions.size = self.vote_extensions.unwrap_or_default();
        config.test.max_retain_blocks = self.max_retain_blocks;

        if let Some(vote_sync_mode) = self.vote_sync_mode {
            config.consensus.vote_sync.mode = vote_sync_mode;
        }
    }
}
