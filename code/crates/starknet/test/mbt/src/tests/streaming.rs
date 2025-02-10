use bytes::Bytes;
use glob::glob;
use malachitebft_engine::util::streaming::StreamId;
use malachitebft_peer::PeerId;

use crate::{streaming::State, utils::*};

pub mod runner;
pub mod utils;

const SHA2_256: u64 = 0x12;

#[test]
fn test_mbt_part_streaming_specified_traces() {
    let temp_dir = tempfile::TempDir::with_prefix("informalsystems-malachitebft-part-streaming")
        .expect("Failed to create temp dir");
    let temp_path = temp_dir.path().to_owned();

    if std::env::var("KEEP_TEMP").is_ok() {
        std::mem::forget(temp_dir);
    }

    let quint_seed = quint_seed();

    print!("{}\n", temp_path.to_string_lossy());
    generate_test_traces(
        "block-streaming/part_stream.qnt",
        &temp_path.to_string_lossy(),
        quint_seed,
    );

    for json_fixture in glob(&format!("{}/*.itf.json", temp_path.display()))
        .expect("Failed to read glob pattern")
        .flatten()
    {
        println!(
            "🚀 Running trace {:?}",
            json_fixture.file_name().unwrap().to_str().unwrap()
        );

        let json = std::fs::read_to_string(&json_fixture).unwrap();
        let trace = itf::trace_from_str::<State>(&json).unwrap();

        let hash = multihash::Multihash::<64>::wrap(SHA2_256, b"PeerId").unwrap();
        let peer_id = PeerId::from_multihash(hash).unwrap();

        let streaming_runner = runner::StreamingRunner::new(peer_id, StreamId::new(Bytes::new()));
        trace.run_on(streaming_runner).unwrap();
    }
}

#[test]
fn test_mbt_part_streaming_random_traces() {
    let temp_dir = tempfile::TempDir::with_prefix("informalsystems-malachitebft-part-streaming")
        .expect("Failed to create temp dir");
    let temp_path = temp_dir.path().to_owned();

    if std::env::var("KEEP_TEMP").is_ok() {
        std::mem::forget(temp_dir);
    }

    let quint_seed = quint_seed();

    print!("{}\n", temp_path.to_string_lossy());
    generate_random_traces(
        "block-streaming/part_stream.qnt",
        &temp_path.to_string_lossy(),
        quint_seed,
        // current quint spec has 4 message parts in tests so there are 24 (4!) possible traces
        // given that duplicate messages case is not covered in the spec
        24,
    );

    for json_fixture in glob(&format!("{}/*.itf.json", temp_path.display()))
        .expect("Failed to read glob pattern")
        .flatten()
    {
        println!(
            "🚀 Running trace {:?}",
            json_fixture.file_name().unwrap().to_str().unwrap()
        );

        let json = std::fs::read_to_string(&json_fixture).unwrap();
        let trace = itf::trace_from_str::<State>(&json).unwrap();

        let hash = multihash::Multihash::<64>::wrap(SHA2_256, b"PeerId").unwrap();
        let peer_id = PeerId::from_multihash(hash).unwrap();

        let streaming_runner = runner::StreamingRunner::new(peer_id, StreamId::new(Bytes::new()));
        trace.run_on(streaming_runner).unwrap();
    }
}
