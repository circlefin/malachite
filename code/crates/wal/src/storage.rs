use std::io::{self, Read, Seek, Write};
use std::path::Path;

/// Operations that the backing storage for the Write-Ahead Log must implement.
///
/// This is mainly used to exercise various failure scenarios in tests,
/// and should otherwise not be used in production code.
///
/// Users are instead encouraged to to use the default [`File`](std::fs::File)-based
/// implementation at [`crate::Log`].
pub trait Storage: Read + Write + Seek + Sized {
    type OpenOptions;

    /// Open the backing storage for the Write-Ahead Log at the given path.
    fn open_with(path: impl AsRef<Path>, options: Self::OpenOptions) -> io::Result<Self>;

    /// Returns the size of the file in bytes.
    fn size_bytes(&self) -> io::Result<u64>;

    /// Truncates the file to the specified size.
    fn truncate_to(&mut self, size: u64) -> io::Result<()>;

    /// Synchronizes all in-memory data to the underlying storage device.
    fn sync_all(&mut self) -> io::Result<()>;
}
