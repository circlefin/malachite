use std::borrow::Cow;
use std::sync::{Arc, OnceLock, RwLock};

pub use prometheus_client::registry::Registry;

#[derive(Clone)]
pub struct SharedRegistry {
    moniker: Option<String>,
    registry: Arc<RwLock<Registry>>,
}

impl SharedRegistry {
    pub fn new(registry: Registry, moniker: Option<String>) -> Self {
        Self {
            moniker,
            registry: Arc::new(RwLock::new(registry)),
        }
    }

    pub fn global() -> &'static Self {
        global_registry()
    }

    pub fn with_moniker(&self, moniker: impl Into<String>) -> Self {
        Self {
            moniker: Some(moniker.into()),
            registry: Arc::clone(&self.registry),
        }
    }

    pub fn with_prefix<A>(&self, prefix: impl AsRef<str>, f: impl FnOnce(&mut Registry) -> A) -> A {
        if let Some(moniker) = &self.moniker {
            self.write(|reg| {
                f(reg
                    .sub_registry_with_prefix(prefix)
                    .sub_registry_with_label((
                        Cow::Borrowed("moniker"),
                        Cow::Owned(moniker.to_string()),
                    )))
            })
        } else {
            self.write(|reg| f(reg.sub_registry_with_prefix(prefix)))
        }
    }

    fn read<A>(&self, f: impl FnOnce(&Registry) -> A) -> A {
        f(&self.registry.read().expect("poisoned lock"))
    }

    fn write<A>(&self, f: impl FnOnce(&mut Registry) -> A) -> A {
        f(&mut self.registry.write().expect("poisoned lock"))
    }
}

fn global_registry() -> &'static SharedRegistry {
    static REGISTRY: OnceLock<SharedRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| SharedRegistry::new(Registry::default(), None))
}

pub fn export<W: core::fmt::Write>(writer: &mut W) {
    use prometheus_client::encoding::text::encode;

    SharedRegistry::global().read(|registry| encode(writer, registry).unwrap())
}
