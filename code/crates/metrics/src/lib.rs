// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::sync::{Arc, Mutex, OnceLock};

pub use prometheus_client::metrics::counter::Counter;
pub use prometheus_client::metrics::family::Family;
pub use prometheus_client::metrics::gauge::Gauge;
pub use prometheus_client::metrics::histogram::{linear_buckets, Histogram};
pub use prometheus_client::registry::Registry;

#[derive(Clone)]
pub struct SharedRegistry(Arc<Mutex<Registry>>);

impl SharedRegistry {
    pub fn new(registry: Registry) -> Self {
        Self(Arc::new(Mutex::new(registry)))
    }

    pub fn global() -> &'static Self {
        global_registry()
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, Registry> {
        self.0.lock().unwrap()
    }

    pub fn with<A>(&self, f: impl FnOnce(&mut Registry) -> A) -> A {
        f(&mut self.lock())
    }

    pub fn with_prefix<A>(&self, prefix: impl AsRef<str>, f: impl FnOnce(&mut Registry) -> A) -> A {
        f(self.lock().sub_registry_with_prefix(prefix))
    }
}

fn global_registry() -> &'static SharedRegistry {
    static REGISTRY: OnceLock<SharedRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| SharedRegistry::new(Registry::default()))
}
pub fn export<W: core::fmt::Write>(writer: &mut W) {
    use prometheus_client::encoding::text::encode;

    SharedRegistry::global().with(|registry| encode(writer, registry).unwrap())
}
