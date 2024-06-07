use std::sync::{Arc, Mutex, OnceLock};

pub use prometheus_client::metrics::counter::Counter;
pub use prometheus_client::metrics::family::Family;
pub use prometheus_client::metrics::gauge::Gauge;
pub use prometheus_client::metrics::histogram::{linear_buckets, Histogram};
pub use prometheus_client::registry::Registry;

fn global_registry() -> &'static Arc<Mutex<Registry>> {
    static REGISTRY: OnceLock<Arc<Mutex<Registry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(Mutex::new(Registry::default())))
}

pub fn with_global_registry<A>(f: impl FnOnce(&mut Registry) -> A) -> A {
    f(&mut global_registry().lock().unwrap())
}

pub fn with_registry_prefixed<A>(prefix: impl AsRef<str>, f: impl FnOnce(&mut Registry) -> A) -> A {
    with_global_registry(|registry| f(registry.sub_registry_with_prefix(prefix)))
}

pub fn export<W: core::fmt::Write>(writer: &mut W) {
    with_global_registry(|registry| {
        prometheus_client::encoding::text::encode(writer, registry).unwrap()
    })
}
