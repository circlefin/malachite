// TODO: Enforce proper documentation
// #![warn(
//     missing_docs,
//     clippy::empty_docs,
//     clippy::missing_errors_doc,
//     rustdoc::broken_intra_doc_links,
//     rustdoc::missing_crate_level_docs,
//     rustdoc::missing_doc_code_examples
// )]

pub use malachite_app as app;

pub mod connector;
pub mod spawn;

mod channel;
pub use channel::{AppMsg, Channels, ConsensusMsg, NetworkMsg, Reply};

mod run;
pub use run::run;
