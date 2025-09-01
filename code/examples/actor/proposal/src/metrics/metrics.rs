use std::ops::Deref;
use std::sync::Arc;

use malachitebft_metrics::prometheus::metrics::counter::Counter;
use malachitebft_metrics::prometheus::metrics::histogram::{linear_buckets, Histogram};
use malachitebft_metrics::SharedRegistry;

#[derive(Clone, Debug)]
pub struct Metrics(Arc<Inner>);

impl Deref for Metrics {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct Inner {
    /// Number of blocks finalized
    pub finalized_blocks: Counter,

    /// Number of transactions finalized
    pub finalized_txes: Counter,

    /// Block size in terms of # of transactions
    pub block_tx_count: Histogram,

    /// Size of each block in bytes
    pub block_size_bytes: Histogram,
}

impl Metrics {
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            finalized_blocks: Counter::default(),
            finalized_txes: Counter::default(),
            block_tx_count: Histogram::new(linear_buckets(0.0, 32.0, 128)),
            block_size_bytes: Histogram::new(linear_buckets(0.0, 64.0 * 1024.0, 128)),
        }))
    }

    pub fn register(registry: &SharedRegistry) -> Self {
        let metrics = Self::new();

        registry.with_prefix("app_actor_proposal", |registry| {
            registry.register(
                "finalized_blocks",
                "Number of blocks finalized",
                metrics.finalized_blocks.clone(),
            );

            registry.register(
                "finalized_txes",
                "Number of transactions finalized",
                metrics.finalized_txes.clone(),
            );

            registry.register(
                "block_tx_count",
                "Block size in terms of # of transactions",
                metrics.block_tx_count.clone(),
            );

            registry.register(
                "block_size_bytes",
                "Size of each block in bytes",
                metrics.block_size_bytes.clone(),
            );
        });

        metrics
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
