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
pub use node::Node;

pub mod types;

mod spawn;
pub use spawn::{
    spawn_block_sync_actor, spawn_consensus_actor, spawn_gossip_consensus_actor, spawn_wal_actor,
};
