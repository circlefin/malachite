use glob::glob;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::streaming::State;
use crate::utils::{generate_test_traces, quint_seed};

pub mod runner;

const RANDOM_SEED: u64 = 0x42;

// generates ITF traces from the Quint spec and runs the traces as Rust tests
#[test]
fn test_itf() {
    let temp_dir = tempfile::TempDir::with_prefix("informalsystems-malachitebft-part-streaming")
        .expect("Failed to create temp dir");
    let temp_path = temp_dir.path().to_owned();

    if std::env::var("KEEP_TEMP").is_ok() {
        std::mem::forget(temp_dir);
    }

    let quint_seed = quint_seed();

    print!("{}\n", temp_path.to_string_lossy());
    generate_test_traces(
        "starknet/block-streaming/part_stream.qnt",
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

        // For debugging
        print!("\n{:#?}\n", trace);

        //TODO:
        // let mut rng = StdRng::seed_from_u64(RANDOM_SEED);

        // // Build mapping from model addresses to real addresses
        // let address_map = utils::build_address_map(&trace, &mut rng);

        // let consensus_runner = ConsensusRunner::new(address_map);
        // trace.run_on(consensus_runner).unwrap();
    }
}
