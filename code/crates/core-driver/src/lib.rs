//! Driver for the state machine of the Malachite consensus engine

#![forbid(unsafe_code)]
#![deny(trivial_casts, trivial_numeric_casts)]
#![warn(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
// no_std compatibility
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]

extern crate alloc;

mod driver;
mod error;
mod input;
mod mux;
mod output;
mod proposal_keeper;

pub use driver::Driver;
pub use error::Error;
pub use input::Input;
pub use output::Output;

pub use malachitebft_core_votekeeper::ThresholdParams;
