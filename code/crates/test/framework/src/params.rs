use bytesize::ByteSize;

use malachitebft_config::{PubSubProtocol, ValuePayload};
use malachitebft_test_app::config::Config;

#[derive(Copy, Clone, Debug)]
pub struct TestParams {
    pub enable_value_sync: bool,
    pub parallel_requests: usize,
    pub protocol: PubSubProtocol,
    pub block_size: ByteSize,
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub vote_extensions: Option<ByteSize>,
    pub value_payload: ValuePayload,
    pub max_retain_blocks: usize,
    pub stable_block_times: bool,
}

impl Default for TestParams {
    fn default() -> Self {
        Self {
            enable_value_sync: false,
            parallel_requests: 1,
            protocol: PubSubProtocol::default(),
            block_size: ByteSize::mib(1),
            tx_size: ByteSize::kib(1),
            txs_per_part: 256,
            vote_extensions: None,
            value_payload: ValuePayload::ProposalAndParts,
            max_retain_blocks: 50,
            stable_block_times: true,
        }
    }
}

impl TestParams {
    pub fn apply_to_config(&self, config: &mut Config) {
        config.value_sync.enabled = self.enable_value_sync;
        config.value_sync.parallel_requests = self.parallel_requests;
        config.consensus.p2p.protocol = self.protocol;
        config.consensus.value_payload = self.value_payload;
        config.test.max_block_size = self.block_size;
        config.test.txs_per_part = self.txs_per_part;
        config.test.vote_extensions.enabled = self.vote_extensions.is_some();
        config.test.vote_extensions.size = self.vote_extensions.unwrap_or_default();
        config.test.max_retain_blocks = self.max_retain_blocks;
        config.test.stable_block_times = self.stable_block_times;
    }
}
