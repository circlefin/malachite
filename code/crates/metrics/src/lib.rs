use std::sync::{Arc, Mutex, OnceLock};

pub use prometheus_client::metrics::counter::Counter;
pub use prometheus_client::metrics::family::Family;
pub use prometheus_client::metrics::gauge::Gauge;
pub use prometheus_client::metrics::histogram::{linear_buckets, Histogram};
pub use prometheus_client::registry::Registry;

pub fn global_registry() -> &'static Arc<Mutex<Registry>> {
    static REGISTRY: OnceLock<Arc<Mutex<Registry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(Mutex::new(Registry::default())))
}

pub fn with_registry<A>(f: impl FnOnce(&mut Registry) -> A) -> A {
    f(&mut global_registry().lock().unwrap())
}

pub fn export<W: core::fmt::Write>(writer: &mut W) {
    let registry = global_registry().lock().unwrap();
    prometheus_client::encoding::text::encode(writer, &registry).unwrap();
}
