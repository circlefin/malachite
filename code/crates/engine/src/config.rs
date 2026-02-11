use malachitebft_config::{ConsensusConfig, P2pConfig, ValuePayload};

/// Engine-internal consensus configuration.
///
/// This wraps the user-facing [`ConsensusConfig`] with fields
/// that are derived at startup rather than set by the operator.
#[derive(Clone, Debug)]
pub struct EngineConsensusConfig {
    /// Enable consensus protocol participation
    pub enabled: bool,

    /// P2P configuration options
    pub p2p: P2pConfig,

    /// Message types that can carry values
    pub value_payload: ValuePayload,

    /// Size of the consensus input queue.
    ///
    /// Derived from `sync.parallel_requests * sync.batch_size` at startup.
    pub queue_capacity: usize,
}

impl EngineConsensusConfig {
    /// Build from the user-facing config plus a computed queue capacity.
    pub fn new(cfg: &ConsensusConfig, queue_capacity: usize) -> Self {
        Self {
            enabled: cfg.enabled,
            p2p: cfg.p2p.clone(),
            value_payload: cfg.value_payload,
            queue_capacity,
        }
    }
}
