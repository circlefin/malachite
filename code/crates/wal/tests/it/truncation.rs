use std::fs::OpenOptions;
use std::io::{self, Seek, SeekFrom};
use std::path::Path;
use std::sync::LazyLock;

use testdir::{NumberedDir, NumberedDirBuilder};

use arc_malachitebft_wal::ext::*;
use arc_malachitebft_wal::log::constants::*;
use arc_malachitebft_wal::Log;

static TESTDIR: LazyLock<NumberedDir> =
    LazyLock::new(|| NumberedDirBuilder::new("wal".to_string()).create().unwrap());

macro_rules! testwal {
    ($e:expr) => {{
        let module_path = ::std::module_path!();
        let test_name = ::testdir::private::extract_test_name(&module_path);
        let subdir_path = ::std::path::Path::new(&module_path.replace("::", "/")).join(&test_name);
        TESTDIR
            .create_subdir(subdir_path)
            .unwrap()
            .join(format!("wal{}.log", $e))
    }};
    () => {{
        testwal!("")
    }};
}

#[test]
fn truncate_on_corruption_at_0() -> io::Result<()> {
    corrupt_and_truncate_at(0)
}
#[test]
fn truncate_on_corruption_at_1() -> io::Result<()> {
    corrupt_and_truncate_at(1)
}
#[test]
fn truncate_on_corruption_at_2() -> io::Result<()> {
    corrupt_and_truncate_at(2)
}
#[test]
fn truncate_on_corruption_at_3() -> io::Result<()> {
    corrupt_and_truncate_at(3)
}
#[test]
fn truncate_on_corruption_at_4() -> io::Result<()> {
    corrupt_and_truncate_at(4)
}
#[test]
fn truncate_on_corruption_at_5() -> io::Result<()> {
    corrupt_and_truncate_at(5)
}
#[test]
fn truncate_on_corruption_at_6() -> io::Result<()> {
    corrupt_and_truncate_at(6)
}
#[test]
fn truncate_on_corruption_at_7() -> io::Result<()> {
    corrupt_and_truncate_at(7)
}
#[test]
fn truncate_on_corruption_at_8() -> io::Result<()> {
    corrupt_and_truncate_at(8)
}
#[test]
fn truncate_on_corruption_at_9() -> io::Result<()> {
    corrupt_and_truncate_at(9)
}

fn corrupt_and_truncate_at(idx: usize) -> io::Result<()> {
    let path = testwal!(idx);
    let entry_count = 5;

    // Setup a valid WAL with LEN entries
    setup_valid_wal(&path, entry_count)?;

    // Corrupt the CRC of entry `idx`
    if idx < entry_count {
        corrupt_wal_entry_crc(&path, idx)?;
    }

    // Reopen WAL and iterate entries
    let mut wal = Log::open(&path)?;

    // Verify WAL state before truncation
    verify_wal_state_pre_fix(&mut wal, idx, entry_count);

    // Truncate the WAL to the valid entries
    wal.truncate(idx as u64)?;

    // Verify WAL recovery
    let expected_len = idx.min(entry_count);
    verify_wal_recovery(&mut wal, expected_len)?;

    Ok(())
}

/// Sets up a valid WAL file with the specified number of entries.
fn setup_valid_wal(path: &Path, count: usize) -> io::Result<()> {
    let mut wal = Log::open(path)?;
    for i in 0..count {
        wal.append(format!("entry{i}").as_bytes())?;
    }
    wal.flush()
}

/// Navigates the file structure to corrupt a specific CRC.
/// This encapsulates the "fragile" byte-logic in one place.
fn corrupt_wal_entry_crc(path: &Path, idx: usize) -> io::Result<()> {
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;

    // Header offset (Version + Sequence)
    file.seek(SeekFrom::Start(HEADER_SIZE))?;

    for _ in 0..idx {
        read_u8(&mut file)?; // skip compression flag
        let entry_len = read_u64(&mut file)?;
        read_u32(&mut file)?; // skip CRC
        file.seek(SeekFrom::Current(entry_len as i64))?; // skip entry data
    }

    // Skip compression flag and length to reach the CRC
    read_u8(&mut file)?;
    read_u64(&mut file)?;

    // Corrupt the CRC
    write_u32(&mut file, 0xDEADBEEF)?;

    // Write changes to disk
    file.sync_all()
}

/// Verifies the WAL iterator behavior before the fix.
fn verify_wal_state_pre_fix(wal: &mut Log, corrupt_idx: usize, total: usize) {
    let results: Vec<_> = wal.iter().unwrap().collect();
    let expected_len = (corrupt_idx + 1).min(total);

    assert_eq!(
        results.len(),
        expected_len,
        "Iterator stopped at wrong point"
    );

    if corrupt_idx < total {
        assert!(
            results[corrupt_idx].is_err(),
            "Expected CRC error at index {corrupt_idx}"
        );
    }
}

/// Verifies that the WAL contains the expected number of valid entries.
fn verify_wal_recovery(wal: &mut Log, expected_len: usize) -> io::Result<()> {
    assert_eq!(wal.len(), expected_len);

    let entries: Vec<Vec<u8>> = wal.iter()?.collect::<Result<_, _>>()?;
    assert_eq!(entries.len(), expected_len);

    for (i, data) in entries.iter().enumerate() {
        assert_eq!(data, format!("entry{i}").as_bytes());
    }
    Ok(())
}
