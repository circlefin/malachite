#![doc = include_str!("../README.md")]
#![allow(rustdoc::private_intra_doc_links)]

mod prelude;

mod input;
pub use input::Input;

mod state;
pub use state::State;

mod error;
pub use error::Error;

mod params;
pub use params::{Params, ThresholdParams};

#[doc(hidden)]
pub use params::HIDDEN_LOCK_ROUND;

mod effect;
pub use effect::{Effect, Resumable, Resume};

mod types;
pub use types::*;

mod full_proposal;
mod macros;
mod util;

mod ser;

// Only used in macros
#[doc(hidden)]
pub mod gen;

// Only used in macros
mod handle;
#[doc(hidden)]
pub use handle::handle;

// Only used internally, but needs to be exposed for tests
#[doc(hidden)]
pub use full_proposal::{FullProposal, FullProposalKeeper};

// Used in macros
#[doc(hidden)]
pub use tracing;
