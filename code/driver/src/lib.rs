//! Driver for the state machine of the Malachite consensus engine

#![no_std]
#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

mod driver;
mod error;
mod input;
mod mux;
mod output;
mod util;

pub use driver::Driver;
pub use error::Error;
pub use input::Input;
pub use output::Output;
pub use util::Validity;
