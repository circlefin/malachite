mod registry;
pub use registry::{export, Registry, SharedRegistry};

mod metrics;
pub use metrics::Metrics;

pub mod prometheus {
    pub use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
    pub use prometheus_client::metrics::counter::Counter;
    pub use prometheus_client::metrics::family::Family;
    pub use prometheus_client::metrics::gauge::Gauge;
    pub use prometheus_client::metrics::histogram::{linear_buckets, Histogram};
    pub use prometheus_client::registry::Registry;
}
