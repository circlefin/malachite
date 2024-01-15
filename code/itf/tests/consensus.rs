#[path = "consensus/runner.rs"]
pub mod runner;
#[path = "consensus/utils.rs"]
pub mod utils;

use glob::glob;
use rand::rngs::StdRng;
use rand::SeedableRng;

use malachite_itf::consensus::State;
use malachite_itf::utils::generate_traces;

use runner::ConsensusRunner;

const RANDOM_SEED: u64 = 0x42;

#[test]
fn test_itf() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let quint_seed = option_env!("QUINT_SEED")
        // use inspect when stabilized
        .map(|x| {
            println!("using QUINT_SEED={}", x);
            x
        })
        .or(Some("118"))
        .and_then(|x| x.parse::<u64>().ok())
        .filter(|&x| x != 0)
        .expect("invalid random seed for quint");

    generate_traces(
        "tests/consensus/consensusTest.qnt",
        &temp_dir.path().to_string_lossy(),
        quint_seed,
    );

    for json_fixture in glob(&format!("{}/*.itf.json", temp_dir.path().display()))
        .expect("Failed to read glob pattern")
        .flatten()
    {
        println!(
            "🚀 Running trace {:?}",
            json_fixture.file_name().unwrap().to_str().unwrap()
        );

        let json = std::fs::read_to_string(&json_fixture).unwrap();
        let trace = itf::trace_from_str::<State>(&json).unwrap();

        let mut rng = StdRng::seed_from_u64(RANDOM_SEED);

        // Build mapping from model addresses to real addresses
        let address_map = utils::build_address_map(&trace, &mut rng);

        let consensus_runner = ConsensusRunner { address_map };

        trace.run_on(consensus_runner).unwrap();
    }
}
