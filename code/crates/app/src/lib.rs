// TODO: Enforce proper documentation
// #![warn(
//     missing_docs,
//     clippy::empty_docs,
//     clippy::missing_errors_doc,
//     rustdoc::broken_intra_doc_links,
//     rustdoc::missing_crate_level_docs,
//     rustdoc::missing_doc_code_examples
// )]

mod node;
pub use node::{EngineHandle, Node, NodeHandle};

pub mod part_store;
pub mod spawn;
pub mod types;

pub mod events {
    pub use malachitebft_engine::util::events::{RxEvent, TxEvent};
}

pub mod streaming {
    pub use malachitebft_engine::util::streaming::*;
}

pub mod consensus {
    pub use malachitebft_core_consensus::*;
}

pub mod metrics {
    pub use malachitebft_metrics::*;
}
