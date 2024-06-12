#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod host;
pub use host::Host;

pub mod mock;
