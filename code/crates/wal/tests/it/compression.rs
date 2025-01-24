use std::io;
use std::sync::LazyLock;

use testdir::{NumberedDir, NumberedDirBuilder};

use informalsystems_malachitebft_wal::Log;

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
    let temp = testwal!();

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
