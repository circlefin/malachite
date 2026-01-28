use std::fs;
use std::io;
use std::sync::LazyLock;
use std::time::Instant;

use rand::{thread_rng, Rng};
use testdir::NumberedDir;
use testdir::NumberedDirBuilder;

use informalsystems_malachitebft_wal::*;

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

const KB: usize = 1024;
const MB: usize = 1024 * KB;

/// Test configuration for customizable stress tests
struct StressTestConfig {
    num_entries: usize,
    sync_interval: usize,
}

#[test]
fn large_number_of_entries() -> io::Result<()> {
    let path = testwal!();

    let config = StressTestConfig {
        num_entries: 1_000_000, // 1 million entries
        sync_interval: 1000,    // Sync every 1000 entries
    };

    let start = Instant::now();
    let mut wal = Log::open(&path)?;

    println!("Starting large number of entries test...");

    // Write entries
    for i in 0..config.num_entries {
        let entry = format!("entry-{i}");
        wal.append(entry.as_bytes())?;

        if i % config.sync_interval == 0 {
            wal.flush()?;
            if i % 100_000 == 0 {
                println!("Wrote {i} entries...");
            }
        }
    }

    wal.flush()?;

    let write_duration = start.elapsed();
    println!("Write phase completed in {write_duration:?}");

    // Verify entries
    let start = Instant::now();
    let entries: Vec<Vec<u8>> = wal.iter()?.collect::<io::Result<Vec<_>>>()?;

    assert_eq!(entries.len(), config.num_entries);

    for (i, entry) in entries.iter().enumerate() {
        let expected = format!("entry-{i}");
        assert_eq!(entry, expected.as_bytes());
    }

    let read_duration = start.elapsed();
    println!("Read phase completed in {read_duration:?}");

    // Report statistics
    let file_size = fs::metadata(&path)?.len();
    println!("WAL Statistics:");
    println!("  Total entries: {}", config.num_entries);
    println!("  File size: {:.2} MB", file_size as f64 / MB as f64);
    println!(
        "  Write throughput: {:.2} entries/sec ({:.2} MB/sec)",
        config.num_entries as f64 / write_duration.as_secs_f64(),
        file_size as f64 / MB as f64 / write_duration.as_secs_f64()
    );
    println!(
        "  Read throughput: {:.2} entries/sec ({:.2} MB/sec)",
        config.num_entries as f64 / read_duration.as_secs_f64(),
        file_size as f64 / MB as f64 / read_duration.as_secs_f64()
    );

    Ok(())
}

#[test]
#[ignore]
fn entry_sizes() -> io::Result<()> {
    let path = testwal!();

    #[allow(clippy::identity_op)]
    let entry_sizes = vec![
        1 * KB,   // 1 KiB
        10 * KB,  // 10 KiB
        100 * KB, // 100 KiB
        500 * KB, // 500 KiB
        1 * MB,   // 1 MiB
        10 * MB,  // 10 MiB
    ];

    let mut wal = Log::open(&path)?;

    println!("Starting entry sizes test...");

    for &size in &entry_sizes {
        // Create entry with random data
        let entry: Vec<u8> = (0..size).map(|_| thread_rng().gen::<u8>()).collect();

        let start = Instant::now();

        if size >= MB {
            println!("Writing entry of size: {} MB", size / MB);
        } else {
            println!("Writing entry of size: {} KB", size / KB);
        }

        // Write entry
        wal.append(&entry)?;
        wal.flush()?;

        let duration = start.elapsed();
        println!("  Write completed in {duration:?}");
        println!(
            "  Throughput: {:.2} MB/s",
            size as f64 / MB as f64 / duration.as_secs_f64()
        );
    }

    // Verify entries
    let start = Instant::now();
    let entries: Vec<Vec<u8>> = wal.iter()?.collect::<io::Result<Vec<_>>>()?;

    assert_eq!(entries.len(), entry_sizes.len());

    for (entry, &expected_size) in entries.iter().zip(entry_sizes.iter()) {
        assert_eq!(entry.len(), expected_size);
    }

    println!("Read verification completed in {:?}", start.elapsed());

    Ok(())
}
