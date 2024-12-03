#![allow(clippy::bool_assert_comparison)]

use std::fs::OpenOptions;
use std::path::Path;
use std::time::Duration;
use std::{fs, io, str};

use malachite_wal::{Log, Version};

use testdir::testdir;

const ENTRIES_1: &[&str] = &[
    "Hello, world!",
    "Wheeee!",
    "1234567890",
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
    "Done!",
];

const ENTRIES_2: &[&str] = &[
    "Something new",
    "Another thing",
    "And another",
    "Yet another",
];

fn setup_wal(path: &Path, entries: &[&str]) -> io::Result<Log> {
    let mut wal = Log::open(path)?;
    println!("Path: {}", wal.path().display());

    let version = wal.version();
    let sequence = wal.sequence();
    assert_eq!(version, Version::V1);
    assert_eq!(sequence, 0);

    for entry in entries {
        wal.write(entry)?;
    }

    assert_eq!(wal.len(), entries.len());
    assert_eq!(wal.is_empty(), entries.is_empty());

    wal.sync()?;

    Ok(wal)
}

#[test]
fn new_wal() -> io::Result<()> {
    let path = testdir!().join("test.wal");
    let wal = Log::open(path)?;
    println!("Path: {}", wal.path().display());

    assert_eq!(wal.version(), Version::V1);
    assert_eq!(wal.sequence(), 0);
    assert_eq!(wal.len(), 0);
    assert_eq!(wal.is_empty(), true);

    Ok(())
}

#[test]
fn open_empty_wal() -> io::Result<()> {
    let path = testdir!().join("test.wal");

    let wal = setup_wal(&path, &[])?;
    let version = wal.version();
    let sequence = wal.sequence();
    drop(wal);

    let wal = Log::open(&path)?;
    assert_eq!(wal.version(), version);
    assert_eq!(wal.sequence(), sequence);
    assert_eq!(wal.len(), 0);
    assert_eq!(wal.is_empty(), true);

    Ok(())
}

#[test]
fn write_entries() -> io::Result<()> {
    let path = testdir!().join("test.wal");

    setup_wal(&path, ENTRIES_1)?;

    let mut wal = Log::open(&path)?;
    assert_eq!(wal.sequence(), 0);
    assert_eq!(wal.len(), ENTRIES_1.len());
    assert_eq!(wal.is_empty(), false);

    for (actual, &expected) in wal.iter()?.zip(ENTRIES_1) {
        let actual = actual?;

        let text = str::from_utf8(&actual).unwrap();
        println!("Entry: {text}");
        assert_eq!(text, expected);
    }

    Ok(())
}

#[test]
fn restart() -> io::Result<()> {
    let path = testdir!().join("test.wal");

    {
        let mut wal = setup_wal(&path, ENTRIES_1)?;
        wal.restart(1)?;

        assert_eq!(wal.sequence(), 1);
        assert_eq!(wal.len(), 0);

        for entry in ENTRIES_2 {
            wal.write(entry)?;
        }

        wal.sync()?;
    }

    let mut wal = Log::open(&path)?;
    assert_eq!(wal.sequence(), 1);

    for (actual, &expected) in wal.iter()?.zip(ENTRIES_2) {
        let actual = actual?;

        let text = str::from_utf8(&actual).unwrap();
        println!("Entry: {text}");
        assert_eq!(text, expected);
    }

    Ok(())
}
#[test]
fn corrupted_wal() -> io::Result<()> {
    let path = testdir!().join("test.wal");

    // Create and write some entries
    {
        let mut wal = Log::open(&path)?;
        wal.write(b"entry1")?;
        wal.write(b"entry2")?;
        wal.sync()?;
    }

    // Corrupt the file by truncating it in the middle
    {
        let metadata = fs::metadata(&path)?;
        let truncate_len = metadata.len() / 2;
        let file = OpenOptions::new().write(true).open(&path)?;
        file.set_len(truncate_len)?;
    }

    // Reopen and verify it handles corruption gracefully
    let wal = Log::open(&path)?;

    // Should have fewer entries due to corruption
    assert!(wal.len() < 2);

    Ok(())
}

#[test]
fn empty_wal_operations() -> io::Result<()> {
    let path = testdir!().join("test.wal");
    let mut wal = Log::open(&path)?;

    assert!(matches!(wal.first_entry(), Ok(None)));
    assert!(wal.iter()?.next().is_none());

    Ok(())
}

#[test]
fn concurrent_access() -> io::Result<()> {
    use std::thread;

    let path = testdir!().join("test.wal");

    // Write in one thread
    let path_clone = path.clone();
    let write_thread = thread::spawn(move || -> io::Result<()> {
        let mut wal = Log::open(&path_clone)?;
        wal.write(b"thread1")?;
        wal.sync()?;
        std::thread::sleep(Duration::from_millis(100));

        Ok(())
    });

    thread::sleep(std::time::Duration::from_millis(50));

    let wal = Log::open(path);

    assert!(wal
        .unwrap_err()
        .to_string()
        .contains("Failed to acquire exclusive advisory lock"));

    write_thread.join().unwrap()?;

    Ok(())
}
