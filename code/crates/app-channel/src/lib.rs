//! Channel-based interface for Malachite applications.

// TODO: Enforce proper documentation
// #![warn(
//     missing_docs,
//     clippy::empty_docs,
//     clippy::missing_errors_doc,
//     rustdoc::broken_intra_doc_links,
//     rustdoc::missing_crate_level_docs,
//     rustdoc::missing_doc_code_examples
// )]

pub use malachitebft_app as app;

mod connector;
mod spawn;

mod msgs;
pub use msgs::{
    AppMsg, Channels, ConsensusMsg, ConsensusRequest, ConsensusRequestError, NetworkMsg, Reply,
};

mod run;
pub use run::start_engine;
