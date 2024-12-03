#![cfg(all(feature = "compression", not(feature = "force-compression")))]

use std::io;

use malachite_wal::Log;
use testdir::testdir;

const ENTRIES: &[&[u8]] = &[
    &[0; 1000],
    &[1; 2000],
    &[2; 3000],
    &[3; 4000],
    &[4; 5000],
    &[5; 6000],
    &[6; 7000],
    &[7; 8000],
    &[8; 9000],
    &[9; 10000],
];

#[test]
fn large_entries() -> io::Result<()> {
    let temp = testdir!();

    let mut no_compression = Log::open(temp.join("no-compression.wal"))?;
    for entry in ENTRIES {
        no_compression.write(entry)?;
    }

    verify_entries(&mut no_compression, ENTRIES)?;

    let mut compression = Log::open(temp.join("compression.wal"))?;
    for entry in ENTRIES {
        compression.write_compressed(entry)?;
    }

    verify_entries(&mut compression, ENTRIES)?;

    let noncompressed_size = no_compression.size_bytes()?;
    let compressed_size = compression.size_bytes()?;

    println!("Non-compressed size: {:>5} bytes", noncompressed_size);
    println!("    Compressed size: {:>5} bytes", compressed_size);

    assert!(compressed_size < noncompressed_size);

    Ok(())
}

fn verify_entries(wal: &mut Log, entries: &[&[u8]]) -> io::Result<()> {
    assert_eq!(wal.len(), entries.len());

    for (actual, &expected) in wal.iter()?.zip(entries) {
        assert_eq!(actual?, expected);
    }

    Ok(())
}
