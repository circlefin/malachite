//! Common data types and abstractions

#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    // missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]

mod consensus;
mod height;
mod proposal;
mod round;
mod timeout;
mod validator_set;
mod value;
mod vote;

pub type ValueId<C> = <<C as Consensus>::Value as Value>::Id;

pub use consensus::Consensus;
pub use height::Height;
pub use proposal::Proposal;
pub use round::Round;
pub use timeout::{Timeout, TimeoutStep};
pub use validator_set::{Address, PublicKey, Validator, ValidatorSet};
pub use value::Value;
pub use vote::{Vote, VoteType};

pub mod test {
    pub use crate::consensus::test::*;
    pub use crate::height::test::*;
    pub use crate::proposal::test::*;
    pub use crate::validator_set::test::*;
    pub use crate::value::test::*;
    pub use crate::vote::test::*;
}
