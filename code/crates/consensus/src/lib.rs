#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    // missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
// no_std compatibility
// #![cfg_attr(not(feature = "std"), no_std)]
// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

mod msg;
pub use msg::Msg;

mod state;
pub use state::State;

mod error;
pub use error::Error;

pub mod handle;

pub mod gen;

mod effect;
pub use effect::{Effect, Resume};

mod types;
pub use types::*;

mod macros;
mod util;
