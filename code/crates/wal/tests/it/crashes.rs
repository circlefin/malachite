use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::thread;
use std::time::Duration;

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use testdir::{NumberedDir, NumberedDirBuilder};

use informalsystems_malachitebft_wal::log::Log;
use informalsystems_malachitebft_wal::Log as FileLog;
use informalsystems_malachitebft_wal::*;

static TESTDIR: LazyLock<NumberedDir> =
    LazyLock::new(|| NumberedDirBuilder::new("wal".to_string()).create().unwrap());

macro_rules! testdir {
    () => {{
        let module_path = ::std::module_path!();
        let test_name = ::testdir::private::extract_test_name(&module_path);
        let subdir_path = ::std::path::Path::new(&module_path.replace("::", "/")).join(&test_name);
        TESTDIR.create_subdir(subdir_path).unwrap()
    }};
}

macro_rules! testwal {
    () => {{
        testdir!().join("wal.log")
    }};
}

/// Helper struct to simulate failures during writes
#[derive(Debug)]
pub struct FailingFile {
    inner: File,
    fail_after: usize,
    bytes_written: usize,
}

impl FailingFile {
    pub fn new(file: File, fail_after: usize) -> Self {
        Self {
            inner: file,
            fail_after,
            bytes_written: 0,
        }
    }
}

impl Seek for FailingFile {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl Read for FailingFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for FailingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.bytes_written + buf.len() > self.fail_after {
            return Err(io::Error::other("Simulated system failure"));
        }
        let written = self.inner.write(buf)?;
        self.bytes_written += written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Storage for FailingFile {
    type OpenOptions = usize;

    fn open_with(path: impl AsRef<Path>, fail_after: usize) -> io::Result<Self> {
        let inner = <File as Storage>::open_with(path, ())?;
        Ok(Self::new(inner, fail_after))
    }

    fn size_bytes(&self) -> io::Result<u64> {
        self.inner.size_bytes()
    }

    fn truncate_to(&mut self, size: u64) -> io::Result<()> {
        self.inner.truncate_to(size)
    }

    fn sync_all(&mut self) -> io::Result<()> {
        self.inner.sync_all()
    }
}

type FailingLog = Log<FailingFile>;

/// Helper function to verify WAL integrity
fn verify_wal_integrity(path: &Path) -> io::Result<Vec<Vec<u8>>> {
    let mut wal = FileLog::open(path)?;
    let entries = wal.iter()?.collect::<io::Result<Vec<_>>>()?;
    Ok(entries)
}

#[test]
fn system_crash_during_write() -> io::Result<()> {
    let temp_dir = testdir!();

    // Test different crash points
    let crash_points = vec![
        // During header write
        4, // During version write
        8, // During sequence number write
        // During entry write
        14, // During entry length write
        18, // During CRC write
        22, // During data write
    ];

    for crash_point in crash_points {
        let path = temp_dir.join(format!("crash-{crash_point}.wal"));

        // Create an empty normal WAL
        FileLog::open(&path)?;

        // Open WAL with failing file
        let storage = FailingFile::open_with(&path, crash_point)?;
        let mut wal = FailingLog::from_raw_parts(storage, path.clone(), Version::V1, 0, 0);

        // Attempt to write entries
        let result = (|| -> io::Result<()> {
            wal.append(b"entry1")?;
            wal.append(b"entry2")?;
            Ok(())
        })();

        assert!(result.is_err());

        // Drop the WAL to unlock the backing file
        drop(wal);

        // Verify WAL integrity after crash
        let entries = verify_wal_integrity(&path)?;

        // The number of successful entries should be consistent with the crash point
        assert!(entries.len() <= 1);
    }

    Ok(())
}

// Simulate power failure during fsync
struct FailingSync {
    inner: File,
    should_fail: bool,
}

impl Write for FailingSync {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Read for FailingSync {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Seek for FailingSync {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl Storage for FailingSync {
    type OpenOptions = bool;

    fn open_with(path: impl AsRef<Path>, should_fail: bool) -> io::Result<Self> {
        let inner = <File as Storage>::open_with(path, ())?;
        Ok(Self { inner, should_fail })
    }

    fn size_bytes(&self) -> io::Result<u64> {
        self.inner.size_bytes()
    }

    fn truncate_to(&mut self, size: u64) -> io::Result<()> {
        self.inner.truncate_to(size)
    }

    fn sync_all(&mut self) -> io::Result<()> {
        if self.should_fail {
            return Err(io::Error::other("Simulated power failure during fsync"));
        }

        self.inner.sync_all()
    }
}

type FailingSyncLog = Log<FailingSync>;

#[test]
fn power_failure_simulation() -> io::Result<()> {
    let path = testwal!();

    // Create an empty normal WAL
    FileLog::open(&path)?;

    // Test power failure during sync
    {
        // Use `from_raw_parts` to avoid calling `sync` during initialization
        let storage = FailingSync::open_with(&path, true)?;
        let mut wal = FailingSyncLog::from_raw_parts(storage, path.to_owned(), Version::V1, 0, 0);

        wal.append(b"entry1")?;

        assert!(wal.flush().is_err());
    }

    // Verify recovery after power failure
    let entries = verify_wal_integrity(&path)?;
    assert!(entries.is_empty() || entries.len() == 1);

    Ok(())
}

#[test]
fn process_termination() -> io::Result<()> {
    let path = testwal!();
    let path_str = path.to_str().unwrap();

    // Create a separate process that will be terminated
    let child = Command::new(std::env::current_exe()?)
        .arg("--test")
        .arg("wal_write_test")
        .arg(path_str)
        .stdout(Stdio::piped())
        .spawn()?;

    // Give the child process time to start writing
    thread::sleep(Duration::from_millis(100));

    // Terminate the process
    signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGKILL)?;

    // Wait for the process to exit
    let _ = child.wait_with_output();

    // Verify WAL integrity after process termination
    let entries = verify_wal_integrity(&path)?;

    // The WAL should either be empty or contain complete entries
    for entry in entries {
        assert!(!entry.is_empty());
    }

    Ok(())
}

// Helper binary for process termination test
#[test]
fn wal_write_test() {
    if std::env::args().any(|arg| arg == "--test") {
        if let Some(path) = std::env::args().nth(3) {
            let mut wal = FileLog::open(path).unwrap();
            loop {
                // Continuously write entries until terminated
                wal.append(b"test entry").unwrap();
                wal.flush().unwrap();
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[test]
fn concurrent_crash_recovery() -> io::Result<()> {
    let path = testwal!();
    let path_clone = path.clone();

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Writer thread
    let writer_handle = thread::spawn(move || -> io::Result<()> {
        let mut wal = FileLog::open(&path_clone)?;

        while running_clone.load(Ordering::SeqCst) {
            wal.append(b"test entry")?;
            wal.flush()?;
            thread::sleep(Duration::from_millis(10));
        }

        Ok(())
    });

    // Crasher thread
    let path2 = path.clone();
    let crasher_handle = thread::spawn(move || {
        for _ in 0..5 {
            thread::sleep(Duration::from_millis(50));

            // Simulate crash by truncating file
            if let Ok(file) = OpenOptions::new().write(true).open(&path2) {
                let _ = file.set_len(12); // Truncate to header size
            }
        }
        running.store(false, Ordering::SeqCst);
    });

    writer_handle.join().unwrap()?;
    crasher_handle.join().unwrap();

    // Verify final WAL integrity
    let entries = verify_wal_integrity(&path)?;

    // The WAL should be in a consistent state
    for entry in entries {
        assert_eq!(&entry, b"test entry");
    }

    Ok(())
}
