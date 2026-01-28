#![cfg_attr(docsrs, feature(doc_cfg))]

//! Write-Ahead Log (WAL) implementation

mod file;
mod storage;
mod version;

pub mod log;

pub use file::{Log, LogEntry, LogIter};
pub use storage::Storage;
pub use version::Version;

// For use in tests
#[doc(hidden)]
pub mod ext;

// For use in tests
#[doc(hidden)]
pub use log::constants;
