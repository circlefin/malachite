//! Custom error messages for CLI helper functions.
//! This low level implementation allows the developer to choose their own error handling library.
use std::path::PathBuf;

/// Error messages for commands
#[derive(Debug)]
pub enum Error {
    /// Error creating parent directory
    ParentDir(PathBuf),

    /// Error opening file
    OpenFile(PathBuf),

    /// Error writing file
    WriteFile,

    /// Error loading file
    LoadFile(PathBuf),

    /// Error converting to JSON
    ToJSON(String),

    /// Error determining home directory path
    DirPath,

    /// Error joining threads
    Join,
}
