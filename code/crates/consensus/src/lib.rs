#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    // missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
// no_std compatibility
#![cfg_attr(not(feature = "std"), no_std)]
// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

mod input;
pub use input::Input;

mod state;
pub use state::State;

mod error;
pub use error::Error;

mod params;
pub use params::{Params, ThresholdParams};

mod effect;
pub use effect::Effect;

mod types;
pub use types::*;

mod full_proposal;
mod handle;
mod macros;
mod util;

// Only used in macros
#[doc(hidden)]
pub mod gen;

// Only used in macros
#[doc(hidden)]
pub use handle::handle;

// Only used internally, but needs to be exposed for tests
#[doc(hidden)]
pub use full_proposal::{FullProposal, FullProposalKeeper};
