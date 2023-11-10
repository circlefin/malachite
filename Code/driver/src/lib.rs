//! Driver for the state machine of the Malachite consensus engine

#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    // missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod client;
mod driver;
mod event;
mod message;

pub use client::Client;
pub use driver::Driver;
pub use event::Event;
pub use message::Message;
