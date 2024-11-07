use std::sync::{Arc, OnceLock, RwLock};

pub use prometheus_client::registry::Registry;

#[derive(Clone)]
pub struct SharedRegistry(Arc<RwLock<Registry>>);

impl SharedRegistry {
    pub fn new(registry: Registry) -> Self {
        Self(Arc::new(RwLock::new(registry)))
    }

    pub fn global() -> &'static Self {
        global_registry()
    }

    pub fn read<A>(&self, f: impl FnOnce(&Registry) -> A) -> A {
        f(&self.0.read().expect("poisoned lock"))
    }

    pub fn write<A>(&self, f: impl FnOnce(&mut Registry) -> A) -> A {
        f(&mut self.0.write().expect("poisoned lock"))
    }

    pub fn with_prefix<A>(&self, prefix: impl AsRef<str>, f: impl FnOnce(&mut Registry) -> A) -> A {
        self.write(|reg| f(reg.sub_registry_with_prefix(prefix)))
    }
}

fn global_registry() -> &'static SharedRegistry {
    static REGISTRY: OnceLock<SharedRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| SharedRegistry::new(Registry::default()))
}

pub fn export<W: core::fmt::Write>(writer: &mut W) {
    use prometheus_client::encoding::text::encode;

    SharedRegistry::global().read(|registry| encode(writer, registry).unwrap())
}
