use bytesize::ByteSize;

use malachitebft_config::{PubSubProtocol, ValuePayload};
use malachitebft_test_app::config::Config;

#[derive(Clone, Debug)]
pub struct TestParams {
    pub enable_value_sync: bool,
    pub consensus_enabled: bool,
    pub parallel_requests: usize,
    pub batch_size: usize,
    pub protocol: PubSubProtocol,
    pub rpc_max_size: ByteSize,
    pub block_size: ByteSize,
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub vote_extensions: Option<ByteSize>,
    pub value_payload: ValuePayload,
    pub max_retain_blocks: usize,
    pub stable_block_times: bool,
    pub max_response_size: ByteSize,
    pub enable_discovery: bool,
    /// Node IDs that should not be added as persistent peers for other nodes
    /// (simulates nodes that joined after initial network setup)
    pub exclude_from_persistent_peers: Vec<u64>,
}

impl Default for TestParams {
    fn default() -> Self {
        Self {
            enable_value_sync: false,
            consensus_enabled: true,
            parallel_requests: 1,
            batch_size: 1,
            protocol: PubSubProtocol::default(),
            rpc_max_size: ByteSize::mib(2),
            block_size: ByteSize::mib(1),
            tx_size: ByteSize::kib(1),
            txs_per_part: 256,
            vote_extensions: None,
            value_payload: ValuePayload::ProposalAndParts,
            max_retain_blocks: 50,
            stable_block_times: true,
            max_response_size: ByteSize::mib(1),
            enable_discovery: false,
            exclude_from_persistent_peers: Vec::new(),
        }
    }
}

impl TestParams {
    pub fn apply_to_config(&self, config: &mut Config) {
        config.value_sync.enabled = self.enable_value_sync;
        config.value_sync.parallel_requests = self.parallel_requests;
        config.value_sync.batch_size = self.batch_size;
        config.value_sync.max_response_size = self.max_response_size;
        config.consensus.enabled = self.consensus_enabled;
        config.consensus.p2p.protocol = self.protocol;
        config.consensus.p2p.rpc_max_size = self.rpc_max_size;
        config.consensus.value_payload = self.value_payload;
        config.consensus.p2p.discovery.enabled = self.enable_discovery;
        // When discovery is enabled, set reasonable defaults for outbound peers
        if self.enable_discovery {
            config.consensus.p2p.discovery.num_outbound_peers = 3;
        }
        config.test.max_block_size = self.block_size;
        config.test.txs_per_part = self.txs_per_part;
        config.test.vote_extensions.enabled = self.vote_extensions.is_some();
        config.test.vote_extensions.size = self.vote_extensions.unwrap_or_default();
        config.test.max_retain_blocks = self.max_retain_blocks;
        config.test.stable_block_times = self.stable_block_times;
    }
}
