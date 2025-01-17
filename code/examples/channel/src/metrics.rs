use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use malachitebft_app_channel::app::metrics;

use metrics::prometheus::metrics::counter::Counter;
use metrics::prometheus::metrics::gauge::Gauge;
use metrics::prometheus::metrics::histogram::{exponential_buckets, Histogram};
use metrics::SharedRegistry;

#[derive(Clone, Debug)]
pub struct DbMetrics(Arc<Inner>);

impl Deref for DbMetrics {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct Inner {
    /// Size of the database database (bytes)
    db_size: Gauge,

    /// Amount of data written to the database (bytes)
    db_write_bytes: Counter,

    /// Amount of data read from the database (bytes)
    db_read_bytes: Counter,

    /// Amount of key data read from the database (bytes)
    db_key_read_bytes: Counter,

    /// Total number of reads from the database
    db_read_count: Counter,

    /// Total number of writes to the database
    db_write_count: Counter,

    /// Total number of deletions to the database
    db_delete_count: Counter,

    /// Time taken to read from the database (seconds)
    db_read_time: Histogram,

    /// Time taken to write to the database (seconds)
    db_write_time: Histogram,

    /// Time taken to delete from the database (seconds)
    db_delete_time: Histogram,
}

impl Inner {
    pub fn new() -> Self {
        Self {
            db_size: Gauge::default(),
            db_write_bytes: Counter::default(),
            db_read_bytes: Counter::default(),
            db_key_read_bytes: Counter::default(),
            db_read_count: Counter::default(),
            db_write_count: Counter::default(),
            db_delete_count: Counter::default(),
            db_read_time: Histogram::new(exponential_buckets(0.001, 2.0, 10)), // Start from 1ms
            db_write_time: Histogram::new(exponential_buckets(0.001, 2.0, 10)),
            db_delete_time: Histogram::new(exponential_buckets(0.001, 2.0, 10)),
        }
    }
}

impl Default for Inner {
    fn default() -> Self {
        Self::new()
    }
}

impl DbMetrics {
    pub fn new() -> Self {
        Self(Arc::new(Inner::new()))
    }

    pub fn register(registry: &SharedRegistry) -> Self {
        let metrics = Self::new();

        registry.with_prefix("app_channel", |registry| {
            registry.register(
                "db_size",
                "Size of the database (bytes)",
                metrics.db_size.clone(),
            );

            registry.register(
                "db_write_bytes_total",
                "Amount of data written to the database (bytes)",
                metrics.db_write_bytes.clone(),
            );

            registry.register(
                "db_read_bytes_total",
                "Amount of data read from the database (bytes)",
                metrics.db_read_bytes.clone(),
            );

            registry.register(
                "db_key_read_bytes_total",
                "Amount of key data read from the database (bytes)",
                metrics.db_key_read_bytes.clone(),
            );

            registry.register(
                "db_read_count_total",
                "Total number of reads from the database",
                metrics.db_read_count.clone(),
            );

            registry.register(
                "db_write_count_total",
                "Total number of writes to the database",
                metrics.db_write_count.clone(),
            );

            registry.register(
                "db_delete_count_total",
                "Total number of deletions to the database",
                metrics.db_delete_count.clone(),
            );

            registry.register(
                "db_read_time",
                "Time taken to read bytes from the database (seconds)",
                metrics.db_read_time.clone(),
            );

            registry.register(
                "db_write_time",
                "Time taken to write bytes to the database (seconds)",
                metrics.db_write_time.clone(),
            );

            registry.register(
                "db_delete_time",
                "Time taken to delete bytes from the database (seconds)",
                metrics.db_delete_time.clone(),
            );
        });

        metrics
    }

    #[allow(dead_code)]
    pub fn set_db_size(&self, size: usize) {
        self.db_size.set(size as i64);
    }

    pub fn add_write_bytes(&self, bytes: u64) {
        self.db_write_bytes.inc_by(bytes);
        self.db_write_count.inc();
    }

    pub fn add_read_bytes(&self, bytes: u64) {
        self.db_read_bytes.inc_by(bytes);
        self.db_read_count.inc();
    }

    pub fn add_key_read_bytes(&self, bytes: u64) {
        self.db_key_read_bytes.inc_by(bytes);
    }

    pub fn observe_read_time(&self, duration: Duration) {
        self.db_read_time.observe(duration.as_secs_f64());
    }

    pub fn observe_write_time(&self, duration: Duration) {
        self.db_write_time.observe(duration.as_secs_f64());
    }

    pub fn observe_delete_time(&self, duration: Duration) {
        self.db_delete_time.observe(duration.as_secs_f64());
    }
}

impl Default for DbMetrics {
    fn default() -> Self {
        Self::new()
    }
}
