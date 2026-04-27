use std::time::Duration;

/// Trait for store metrics instrumentation.
///
/// Implement this trait to collect metrics on database operations.
/// Use [`NoMetrics`] when no metrics collection is needed.
pub trait StoreMetrics: Send + Sync + 'static {
    fn observe_read_time(&self, _duration: Duration) {}
    fn add_read_bytes(&self, _bytes: u64) {}
    fn add_key_read_bytes(&self, _bytes: u64) {}
    fn observe_write_time(&self, _duration: Duration) {}
    fn add_write_bytes(&self, _bytes: u64) {}
    fn observe_delete_time(&self, _duration: Duration) {}
}

/// No-op metrics implementation.
pub struct NoMetrics;

impl StoreMetrics for NoMetrics {}

impl StoreMetrics for Box<dyn StoreMetrics> {
    fn observe_read_time(&self, duration: Duration) {
        (**self).observe_read_time(duration);
    }
    fn add_read_bytes(&self, bytes: u64) {
        (**self).add_read_bytes(bytes);
    }
    fn add_key_read_bytes(&self, bytes: u64) {
        (**self).add_key_read_bytes(bytes);
    }
    fn observe_write_time(&self, duration: Duration) {
        (**self).observe_write_time(duration);
    }
    fn add_write_bytes(&self, bytes: u64) {
        (**self).add_write_bytes(bytes);
    }
    fn observe_delete_time(&self, duration: Duration) {
        (**self).observe_delete_time(duration);
    }
}
