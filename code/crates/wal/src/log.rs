//! Write-Ahead Log (WAL) implementation, generic over its backing storage.
//!
//! # Warning
//! Not for regular use, use [`crate::Log`] instead.

use std::io::{self, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::ext::{read_u32, read_u64, write_u32, write_u64};
use crate::{Storage, Version};

/// Represents a single entry in the Write-Ahead Log (WAL).
///
/// Each entry has the following format on disk:
///
/// ```text
/// +-----------------+----------------+-----------------+
/// |     Length      |      CRC       |      Data       |
/// |    (4 bytes)    |   (4 bytes)    | ($length bytes) |
/// +-----------------+----------------+-----------------+
/// ```
pub struct LogEntry<'a, S> {
    /// Reference to the parent WAL
    log: &'a mut Log<S>,
}

impl<S> LogEntry<'_, S>
where
    S: Storage,
{
    /// Reads the length field of the current entry
    fn length(&mut self) -> io::Result<u64> {
        read_u64(&mut self.log.storage)
    }

    /// Reads the CRC field of the current entry
    fn crc(&mut self) -> io::Result<u32> {
        read_u32(&mut self.log.storage)
    }

    /// Reads the current entry's data and advances to the next entry.
    /// The entry data is written to the provided writer.
    ///
    /// # Arguments
    /// * `writer` - The writer to output the entry data to
    ///
    /// # Returns
    /// * `Ok(Some(self))` - If there are more entries to read
    /// * `Ok(None)` - If this was the last entry
    /// * `Err` - If an I/O error occurs or the CRC check fails
    pub fn read_to_next<W: Write>(mut self, writer: &mut W) -> io::Result<Option<Self>> {
        let length = self.length()? as usize;
        let expected_crc = self.crc()?;

        let mut data = vec![0; length];
        self.log.storage.read_exact(&mut data)?;

        let actual_crc = compute_crc(&data);

        if expected_crc != actual_crc {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "CRC mismatch"));
        }

        writer.write_all(&data)?;

        let pos = self.log.storage.stream_position()?;
        let len = self.log.storage.size_bytes()?;

        if pos < len {
            Ok(Some(self))
        } else {
            Ok(None)
        }
    }
}

/// Write-Ahead Log (WAL)
///
/// A Write-Ahead Log is a sequential log of records that provides durability and atomicity
/// guarantees by writing changes to disk before they are applied to the main database.
///
/// # Format on disk
///
/// ```text
/// +-----------------+-----------------+-----------------+-----------------+-----------------+
/// |     Version     |     Sequence    |    Entry #1     |       ...       |     Entry #n    |
/// |    (4 bytes)    |    (8 bytes)    |    (variable)   |                 |    (variable)   |
/// +-----------------+-----------------+-----------------+-----------------+-----------------+
/// ```
#[derive(Debug)]
pub struct Log<S> {
    storage: S,
    path: PathBuf,
    version: Version,
    sequence: u64,
    len: usize,
}

const VERSION_LENGTH: u64 = size_of::<Version>() as u64;
const SEQUENCE_LENGTH: u64 = size_of::<u64>() as u64;

/// Length of the WAL header in bytes (version + sequence)
const HEADER_LENGTH: u64 = VERSION_LENGTH + SEQUENCE_LENGTH;

const U32_SIZE: u64 = size_of::<u32>() as u64;
const U64_SIZE: u64 = size_of::<u64>() as u64;

impl<S> Log<S>
where
    S: Storage<OpenOptions = ()>,
{
    /// Opens a Write-Ahead Log file at the specified path.
    ///
    /// If the file already exists, it will be opened and validated.
    /// If the file does not exist, a new one will be created.
    ///
    /// # Arguments
    /// * `path` - Path where the WAL file should be created/opened
    ///
    /// # Returns
    /// * `Ok(Wal)` - Successfully opened/created WAL
    /// * `Err` - If file operations fail or existing WAL is invalid
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_with(path, ())
    }
}

impl<S> Log<S>
where
    S: Storage,
{
    /// Opens a Write-Ahead Log file at the specified path.
    ///
    /// If the file already exists, it will be opened and validated.
    /// If the file does not exist, a new one will be created.
    ///
    /// # Arguments
    /// * `path` - Path where the WAL file should be created/opened
    ///
    /// # Returns
    /// * `Ok(Wal)` - Successfully opened/created WAL
    /// * `Err` - If file operations fail or existing WAL is invalid
    pub fn open_with(path: impl AsRef<Path>, options: S::OpenOptions) -> io::Result<Self> {
        let path = path.as_ref().to_owned();

        let mut storage = S::open_with(&path, options)?;

        let size = storage.size_bytes()?;

        // If file exists and has content
        if size > 0 {
            // Read and validate version number
            let version = Version::try_from(read_u32(&mut storage)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid WAL version"))?;

            // Read sequence number
            let sequence = read_u64(&mut storage).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Failed to read sequence number",
                )
            })?;

            // Track current position and entry count
            let mut pos = HEADER_LENGTH; // Start after header
            let mut len = 0;

            // Scan through entries to validate and count them
            while size.saturating_sub(pos) > U64_SIZE {
                // Read entry length
                let data_length = read_u64(&mut storage)?;

                // Calculate total entry size including CRC
                let Some(entry_length) = data_length.checked_add(U32_SIZE) else {
                    break; // Integer overflow, file is corrupt
                };

                // Check if enough bytes remain for full entry
                if size.saturating_sub(pos) < entry_length {
                    break; // Partial/corrupt entry
                }

                // Skip to next entry
                pos = storage.seek(SeekFrom::Current(entry_length.try_into().unwrap()))?;
                len += 1;
            }

            // Truncate any partial entries at the end
            storage.truncate_to(pos)?;
            storage.sync_all()?;

            return Ok(Self {
                version,
                storage,
                path,
                sequence,
                len,
            });
        }

        // Creating new WAL file
        let version = Version::V1;

        // Write header: version (4 bytes)
        write_u32(&mut storage, version as u32)?;

        // Write header: sequence (8 bytes)
        write_u64(&mut storage, 0)?;

        // Ensure file is exactly header size
        storage.truncate_to(HEADER_LENGTH)?;

        // Ensure header is persisted to disk
        storage.sync_all()?;

        Ok(Self {
            version,
            storage,
            path,
            sequence: 0,
            len: 0,
        })
    }

    /// Writes a new entry to the WAL.
    ///
    /// The entry is appended to the end of the log with length, CRC and data.
    /// If writing fails, the WAL is truncated to remove the partial write.
    ///
    /// # Arguments
    /// * `data` - The data to write as a new WAL entry
    ///
    /// # Returns
    /// * `Ok(())` - Entry was successfully written
    /// * `Err` - If writing fails
    pub fn write(&mut self, data: impl AsRef<[u8]>) -> io::Result<()> {
        let pos = self.storage.seek(SeekFrom::End(0))?;

        let data = data.as_ref();
        let len = data.len();

        let result = || -> io::Result<()> {
            // Write entry length
            write_u64(&mut self.storage, len as u64)?;

            // Write entry CRC
            write_u32(&mut self.storage, compute_crc(data))?;

            // Write entry data
            self.storage.write_all(data)?;

            Ok(())
        }();

        match result {
            Ok(()) => {
                self.len += 1;
                Ok(())
            }
            Err(e) => {
                self.storage.truncate_to(pos)?;
                Err(e)
            }
        }
    }

    /// Returns an the first entry in the WAL if it exists.
    ///
    /// # Returns
    /// * `Ok(Some(WalEntry))` - First entry exists and was retrieved
    /// * `Ok(None)` - WAL is empty
    /// * `Err` - If reading fails or WAL is invalid
    pub fn first_entry(&mut self) -> io::Result<Option<LogEntry<S>>> {
        // IF the file is empty, return an error
        if self.storage.size_bytes()? == 0 {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Empty WAL"));
        }

        // If there are no entries, return None
        if self.len == 0 {
            return Ok(None);
        }

        // Seek to the first entry after the header
        self.storage.seek(SeekFrom::Start(HEADER_LENGTH))?;

        Ok(Some(LogEntry { log: self }))
    }

    /// Returns an iterator over all entries in the WAL.
    ///
    /// # Returns
    /// * `Ok(LogIter)` - Iterator over WAL entries
    /// * `Err` - If reading fails
    pub fn iter(&mut self) -> io::Result<LogIter<S>> {
        Ok(LogIter {
            next: self.first_entry()?,
        })
    }

    /// Restarts the WAL with a new sequence number.
    ///
    /// This truncates all existing entries and resets the WAL to an empty state
    /// with the specified sequence number.
    ///
    /// # Arguments
    /// * `sequence` - New sequence number to start from
    ///
    /// # Returns
    /// * `Ok(())` - WAL was successfully restarted
    /// * `Err` - If file operations fail
    pub fn restart(&mut self, sequence: u64) -> io::Result<()> {
        // Reset sequence number and entry count
        self.sequence = sequence;
        self.len = 0;

        // Seek to start of sequence number
        self.storage.seek(SeekFrom::Start(4))?;

        // Write new sequence number
        write_u64(&mut self.storage, sequence)?;

        // Truncate all entries
        self.storage.truncate_to(HEADER_LENGTH)?;

        // Sync changes to disk
        self.storage.sync_all()?;

        Ok(())
    }

    /// Syncs all written data to disk.
    ///
    /// On UNIX systems, this will call `fsync` to ensure all data is written to disk.
    ///
    /// # Returns
    /// * `Ok(())` - Successfully synced to disk
    /// * `Err` - If sync fails
    pub fn sync(&mut self) -> io::Result<()> {
        self.storage.sync_all()
    }

    /// Build a Write-Ahead Log (WAL) from its raw components.
    ///
    /// # Safety
    /// This is a dangerous function that should not be used directly.
    /// It bypasses important initialization and validation checks.
    /// Instead, use `malachite_wal::file::Log::open` which properly initializes the WAL.
    ///
    /// This function exists primarily for internal use and testing purposes.
    pub fn from_raw_parts(
        file: S,
        path: PathBuf,
        version: Version,
        sequence: u64,
        len: usize,
    ) -> Self {
        Self {
            storage: file,
            path,
            version,
            sequence,
            len,
        }
    }
}

impl<S> Log<S> {
    /// Returns the version of the WAL format.
    pub fn version(&self) -> Version {
        self.version
    }

    /// Returns the current sequence number.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the path to the WAL file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the number of entries in the WAL.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the WAL is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Iterator over entries in a Write-Ahead Log (WAL)
pub struct LogIter<'a, F> {
    /// The next entry to be read from the WAL
    next: Option<LogEntry<'a, F>>,
}

/// Iterator over entries in a Write-Ahead Log (WAL)
///
/// Provides sequential access to entries stored in the WAL.
/// Each iteration returns the data contained in the next entry.
impl<F> Iterator for LogIter<'_, F>
where
    F: Storage,
{
    /// Each iteration returns a Result containing either the entry data as a `Vec<u8>`
    /// or an IO error if reading fails
    type Item = io::Result<Vec<u8>>;

    /// Advances the iterator and returns the next entry's data
    ///
    /// # Returns
    /// * `Some(Ok(Vec<u8>))` - Successfully read entry data
    /// * `Some(Err(e))` - Error occurred while reading entry
    /// * `None` - No more entries to read
    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = Vec::new();
        let next = self.next.take()?;

        match next.read_to_next(&mut buf) {
            Ok(Some(entry)) => {
                self.next = Some(entry);
                Some(Ok(buf))
            }
            Ok(None) => Some(Ok(buf)),
            Err(e) => Some(Err(e)),
        }
    }
}

/// Computes the CRC32 checksum of the provided data
///
/// # Arguments
/// * `data` - The bytes to compute the checksum for
///
/// # Returns
/// The CRC32 checksum as a u32 in big-endian byte order
fn compute_crc(data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(data);
    u32::from_be_bytes(hasher.finalize().to_be_bytes())
}
