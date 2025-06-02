use std::fs::{self, File};
use std::io;
use std::path::Path;

use advisory_lock::{AdvisoryFileLock, FileLockMode};

use crate::storage::Storage;

/// Write-Ahead Log (WAL) backed by a [`File`](std::fs::File)
pub type Log = crate::log::Log<File>;

/// Write-Ahead Log (WAL) entry, backed by a [`File`](std::fs::File)
pub type LogEntry<'a> = crate::log::LogEntry<'a, File>;

/// Iterator over the WAL entries, backed by a [`File`](std::fs::File)
pub type LogIter<'a> = crate::log::LogIter<'a, File>;

impl Storage for File {
    type OpenOptions = ();

    fn open_with(path: impl AsRef<Path>, _: ()) -> io::Result<Self> {
        // Open file with read+write access, create if doesn't exist
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false) // Don't truncate existing file
            .open(&path)?;

        AdvisoryFileLock::try_lock(&file, FileLockMode::Exclusive).map_err(|e| {
            io::Error::other(format!("Failed to acquire exclusive advisory lock: {e}"))
        })?;

        Ok(file)
    }

    fn size_bytes(&self) -> io::Result<u64> {
        File::metadata(self).map(|m| m.len())
    }

    fn truncate_to(&mut self, size: u64) -> io::Result<()> {
        File::set_len(self, size)
    }

    fn sync_all(&mut self) -> io::Result<()> {
        File::sync_all(self)
    }
}
