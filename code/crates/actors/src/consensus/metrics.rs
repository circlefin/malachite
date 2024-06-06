use malachite_metrics::{linear_buckets, Counter, Gauge, Histogram};

#[derive(Clone, Debug)]
pub struct Metrics {
    /// Number of blocks finalized
    pub finalized_blocks: Counter,

    /// Number of transactions finalized
    pub finalized_txes: Counter,

    /// Block size in terms of # of transactions
    pub block_size: Histogram,

    /// Size of each block in bytes
    pub block_bytes: Histogram,

    /// Consensus rounds, ie. how many rounds did each block need to reach finalization
    pub rounds_per_block: Histogram,

    /// Number of connected peers, ie. for each consensus node, how many peers is it connected to)
    pub connected_peers: Gauge,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            finalized_blocks: Counter::default(),
            finalized_txes: Counter::default(),
            block_size: Histogram::new(linear_buckets(0.0, 32.0, 128)),
            block_bytes: Histogram::new(linear_buckets(0.0, 64.0 * 1024.0, 128)),
            rounds_per_block: Histogram::new(linear_buckets(0.0, 1.0, 20)),
            connected_peers: Gauge::default(),
        }
    }

    pub fn register() -> Self {
        let metrics = Self::new();

        let mut registry = malachite_metrics::global_registry().lock().unwrap();

        registry.register(
            "malachite_consensus_finalized_blocks",
            "Number of blocks finalized",
            metrics.finalized_blocks.clone(),
        );

        registry.register(
            "malachite_consensus_finalized_txes",
            "Number of transactions finalized",
            metrics.finalized_txes.clone(),
        );

        registry.register(
            "malachite_consensus_block_size",
            "Block size in terms of # of transactions",
            metrics.block_size.clone(),
        );

        registry.register(
            "malachite_consensus_block_bytes",
            "Size of each block in bytes",
            metrics.block_bytes.clone(),
        );

        registry.register(
            "malachite_consensus_rounds_per_block",
            "Consensus rounds, ie. how many rounds did each block need to reach finalization",
            metrics.rounds_per_block.clone(),
        );

        registry.register(
            "malachite_consensus_connected_peers",
            "Number of connected peers, ie. for each consensus node, how many peers is it connected to",
            metrics.connected_peers.clone(),
        );

        metrics
    }
}
