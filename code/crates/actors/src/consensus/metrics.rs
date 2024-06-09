use std::ops::Deref;
use std::sync::Arc;

use malachite_metrics::{linear_buckets, Counter, Gauge, Histogram, SharedRegistry};

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

    /// Time taken to finalize a block, in seconds
    pub time_per_block: Histogram,

    /// Block size in terms of # of transactions
    pub block_tx_count: Histogram,

    /// Size of each block in bytes
    pub block_size_bytes: Histogram,

    /// Consensus rounds, ie. how many rounds did each block need to reach finalization
    pub rounds_per_block: Histogram,

    /// Number of connected peers, ie. for each consensus node, how many peers is it connected to)
    pub connected_peers: Gauge,

    /// Internal state for measuring time taken to finalize a block
    instant_block_started: Arc<AtomicInstant>,
}

impl Metrics {
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            finalized_blocks: Counter::default(),
            finalized_txes: Counter::default(),
            time_per_block: Histogram::new(linear_buckets(0.0, 1.0, 20)),
            block_tx_count: Histogram::new(linear_buckets(0.0, 32.0, 128)),
            block_size_bytes: Histogram::new(linear_buckets(0.0, 64.0 * 1024.0, 128)),
            rounds_per_block: Histogram::new(linear_buckets(0.0, 1.0, 20)),
            connected_peers: Gauge::default(),
            instant_block_started: Arc::new(AtomicInstant::empty()),
        }))
    }

    pub fn register(registry: &SharedRegistry) -> Self {
        let metrics = Self::new();

        registry.with_prefix("malachite_consensus", |registry| {
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
                "time_per_block",
                "Time taken to finalize a block, in seconds",
                metrics.time_per_block.clone(),
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

            registry.register(
                "rounds_per_block",
                "Consensus rounds, ie. how many rounds did each block need to reach finalization",
                metrics.rounds_per_block.clone(),
            );

            registry.register(
                "connected_peers",
                "Number of connected peers, ie. for each consensus node, how many peers is it connected to",
                metrics.connected_peers.clone(),
            );
        });

        metrics
    }

    pub fn block_start(&self) {
        self.instant_block_started.set_now();
    }

    pub fn block_end(&self) {
        if !self.instant_block_started.is_empty() {
            let elapsed = self.instant_block_started.elapsed().as_secs_f64();
            self.time_per_block.observe(elapsed);

            self.instant_block_started.set_millis(0);
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

#[derive(Default, Debug)]
struct AtomicInstant(AtomicU64);

#[allow(dead_code)]
impl AtomicInstant {
    pub fn now() -> Self {
        Self(AtomicU64::new(Self::now_millis()))
    }

    pub fn empty() -> Self {
        Self(AtomicU64::new(0))
    }

    pub const fn from_millis(millis: u64) -> Self {
        Self(AtomicU64::new(millis))
    }

    pub fn elapsed(&self) -> Duration {
        Duration::from_millis(Self::now_millis() - self.as_millis())
    }

    pub fn as_millis(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    pub fn set_now(&self) {
        self.set_millis(Self::now_millis());
    }

    pub fn set_millis(&self, millis: u64) {
        self.0.store(millis, Ordering::SeqCst);
    }

    pub fn is_empty(&self) -> bool {
        self.as_millis() == 0
    }

    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}
