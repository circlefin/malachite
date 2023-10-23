//! Common data types and abstractions

#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    // missing_docs,
    broken_intra_doc_links,
    private_intra_doc_links,
    variant_size_differences
)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]

mod height;
mod proposal;
mod round;
mod timeout;
mod validator_set;
mod value;
mod vote;

pub use height::*;
pub use proposal::*;
pub use round::*;
pub use timeout::*;
pub use validator_set::*;
pub use value::*;
pub use vote::*;
