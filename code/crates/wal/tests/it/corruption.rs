use std::fs::OpenOptions;
use std::io::{self, Seek, SeekFrom, Write};
use std::sync::LazyLock;

use testdir::{NumberedDir, NumberedDirBuilder};

use informalsystems_malachitebft_wal::ext::*;
use informalsystems_malachitebft_wal::{Log, Version};

static TESTDIR: LazyLock<NumberedDir> =
    LazyLock::new(|| NumberedDirBuilder::new("wal".to_string()).create().unwrap());

macro_rules! testwal {
    () => {{
        let module_path = ::std::module_path!();
        let test_name = ::testdir::private::extract_test_name(&module_path);
        let subdir_path = ::std::path::Path::new(&module_path.replace("::", "/")).join(&test_name);
        TESTDIR.create_subdir(subdir_path).unwrap().join("wal.log")
    }};
}

#[test]
fn corrupted_crc() -> io::Result<()> {
    let path = testwal!();

    // Write initial entries
    {
        let mut wal = Log::open(&path)?;
        wal.append(b"entry1")?;
        wal.append(b"entry2")?;
        wal.flush()?;
    }

    // Corrupt the CRC of the second entry
    {
        let mut file = OpenOptions::new().read(true).write(true).open(&path)?;

        // Skip version (4 bytes) + sequence (8 bytes) + first entry
        file.seek(SeekFrom::Start(12))?;
        read_u8(&mut file)?; // Skip compression flag
        let first_entry_len = read_u64(&mut file)?;
        file.seek(SeekFrom::Current(first_entry_len as i64 + 4))?; // +4 for CRC

        // Now at the start of second entry, skip compression flag and length
        file.seek(SeekFrom::Current(1 + 8))?;

        // Write incorrect CRC
        write_u32(&mut file, 0xdeadbeef)?;
    }

    // Reopen and verify
    {
        let mut wal = Log::open(&path)?;
        let mut entries = wal.iter()?;

        // First entry should be readable
        assert!(entries.next().is_some());

        // Second entry should fail CRC check
        match entries.next() {
            Some(Err(e)) => assert_eq!(e.kind(), io::ErrorKind::InvalidData),
            _ => panic!("Expected CRC error for corrupted entry"),
        }
    }

    Ok(())
}

#[test]
fn incomplete_entries() -> io::Result<()> {
    let path = testwal!();

    // Write initial entries
    {
        let mut wal = Log::open(&path)?;
        wal.append(b"entry1")?;
        wal.append(b"entry2")?;
        wal.flush()?;
    }

    // Truncate file in the middle of the second entry
    {
        let mut file = OpenOptions::new().read(true).write(true).open(&path)?;

        // Skip header
        file.seek(SeekFrom::Start(12))?;

        read_u8(&mut file)?; // Skip compression flag
        let first_entry_len = read_u64(&mut file)?;

        // header + compression flag + length + CRC + data + partial second entry
        let truncate_pos = 12 + 1 + 8 + 4 + first_entry_len + 3;

        // Seek to middle of second entry
        file.set_len(truncate_pos)?;
    }

    // Reopen and verify
    {
        let mut wal = Log::open(&path)?;
        let entries: Vec<_> = wal.iter()?.collect::<Result<Vec<_>, _>>()?;

        // Should only have the first entry
        assert_eq!(entries.len(), 1);
        assert_eq!(&entries[0], b"entry1");
    }

    Ok(())
}

#[test]
fn invalid_version() -> io::Result<()> {
    let path = testwal!();

    // Create WAL file with invalid version
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;

        // Write invalid version
        write_u32(&mut file, 0xFFFFFFFF)?;
        write_u64(&mut file, 0)?; // sequence
    }

    // Attempt to open WAL
    match Log::open(&path) {
        Err(e) => {
            assert_eq!(e.kind(), io::ErrorKind::InvalidData);
            // Verify error message contains version information
            assert!(e.to_string().contains("version"));
        }
        Ok(_) => panic!("Expected error when opening WAL with invalid version"),
    }

    Ok(())
}

#[test]
fn invalid_sequence() -> io::Result<()> {
    let path = testwal!();

    // Create WAL with valid version but corrupted sequence
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;

        write_u32(&mut file, Version::V1 as u32)?;

        // Write partial/corrupted sequence number
        file.write_all(&[0xFF, 0xFF])?; // Only write 2 bytes instead of 8
    }

    // Attempt to open WAL
    match Log::open(&path) {
        Err(e) => {
            assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
        }
        Ok(_) => panic!("Expected error when opening WAL with invalid sequence"),
    }

    Ok(())
}

#[test]
fn multiple_corruptions() -> io::Result<()> {
    let path = testwal!();

    // Create initial WAL with entries
    {
        let mut wal = Log::open(&path)?;
        wal.append(b"entry1")?;
        wal.append(b"entry2")?;
        wal.append(b"entry3")?;
        wal.flush()?;
    }

    // Introduce multiple types of corruption
    {
        let mut file = OpenOptions::new().read(true).write(true).open(&path)?;

        // Corrupt sequence number
        file.seek(SeekFrom::Start(4))?;
        write_u64(&mut file, u64::MAX)?;

        // Corrupt entry length
        file.seek(SeekFrom::Start(12 + 1))?;
        write_u64(&mut file, u64::MAX - 1)?;

        // Corrupt CRC of another entry
        file.seek(SeekFrom::Start(50 + 1))?;
        write_u32(&mut file, 0xdeadbeef)?;
    }

    // Attempt to open and read
    {
        let mut wal = Log::open(&path)?;
        let entries: Vec<_> = wal
            .iter()?
            .take_while(|r| r.is_ok())
            .collect::<Result<Vec<_>, _>>()?;

        // Should have recovered what it could
        assert!(entries.len() < 3);
    }

    Ok(())
}

#[test]
fn zero_length_entries() -> io::Result<()> {
    let path = testwal!();

    // Create WAL with zero-length entry
    {
        let mut wal = Log::open(&path)?;
        wal.append(b"")?;
        wal.append(b"normal entry")?;
        wal.flush()?;
    }

    // Verify reading
    {
        let mut wal = Log::open(&path)?;
        let entries: Vec<_> = wal.iter()?.collect::<Result<Vec<_>, _>>()?;

        assert_eq!(entries.len(), 2);
        assert_eq!(&entries[0], b"");
        assert_eq!(&entries[1], b"normal entry");
    }

    Ok(())
}

#[test]
#[ignore]
fn max_entry_size() -> io::Result<()> {
    let path = testwal!();

    let mut wal = Log::open(&path)?;

    // Try to write an entry that's too large
    let large_entry = vec![0u8; usize::MAX / 2];
    assert!(wal.append(&large_entry).is_err());

    // Verify WAL is still usable
    wal.append(b"normal entry")?;

    let entries: Vec<_> = wal.iter()?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(entries.len(), 1);
    assert_eq!(&entries[0], b"normal entry");

    Ok(())
}

/// This test creates a scenario where a valid entry's length field is corrupted
/// with a value that is invalid, but small enough to fool the original,
/// flawed recovery logic.
///
/// The old logic would incorrectly validate this entry and fail to truncate
/// the log, leading to a corrupt state. The corrected logic must identify
/// that the full entry cannot fit and truncate the file correctly.
#[test]
fn recovery_fails_to_truncate_with_carefully_corrupted_length() -> io::Result<()> {
    let path = testwal!();

    let second_entry_start_pos;

    // Write two valid entries
    {
        let mut wal = Log::open(&path)?;
        wal.append(b"entry1")?;

        // Position after entry 1 is the start of entry 2
        second_entry_start_pos = wal.size_bytes()?;

        wal.append(b"entry2")?;
        wal.flush()?;
    }

    // The total remaining space for the second entry is (total_size - second_entry_start_pos).
    // Let's say this is 19 bytes.
    // We will corrupt the length to a value that is *less than* this, to fool the old check.
    // The actual data + crc for entry2 is 6+4=10 bytes. The header is 13 bytes.
    // Let's corrupt the data_length to 15. The old code would calculate an
    // entry_length of 15+4=19. The check `19 < 19` would be false, and it would
    // incorrectly validate the entry.
    let malicious_data_length = 15u64;

    // Manually corrupt the length field of the *second* entry.
    {
        let mut file = OpenOptions::new().write(true).open(&path)?;
        // Seek to the start of the second entry, then skip the compression flag (1 byte)
        file.seek(SeekFrom::Start(second_entry_start_pos + 1))?;
        // Overwrite the 8-byte length with our malicious value.
        write_u64(&mut file, malicious_data_length)?;
    }

    // Reopen the WAL. The recovery logic should run.
    {
        // With the BUG, Log::open succeeds but wal.len() would be 2.
        // With the FIX, Log::open succeeds and wal.len() is correctly 1.
        let mut wal = Log::open(&path)?;
        assert_eq!(
            wal.len(),
            1,
            "WAL should have recovered only one valid entry"
        );

        let entries = wal.iter()?.collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            entries.len(),
            1,
            "Iterator should yield only the first valid entry"
        );
        assert_eq!(entries[0], b"entry1");
    }

    Ok(())
}
