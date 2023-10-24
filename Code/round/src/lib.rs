//! Per-round consensus state machine

#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    // missing_docs,
    broken_intra_doc_links,
    private_intra_doc_links,
    variant_size_differences
)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]

pub use malachite_common::*;

pub mod events;
pub mod message;
pub mod state;
pub mod state_machine;
