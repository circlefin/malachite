#[path = "votekeeper/runner.rs"]
pub mod runner;
#[path = "votekeeper/utils.rs"]
pub mod utils;

use glob::glob;
use rand::rngs::StdRng;
use rand::SeedableRng;

use malachite_itf::utils::generate_traces;
use malachite_itf::votekeeper::State;

use runner::VoteKeeperRunner;
use utils::{build_address_map, build_public_key_map};

const RANDOM_SEED: u64 = 0x42;

#[test]
fn test_itf() {
    let temp_dir =
        tempfile::TempDir::with_prefix("malachite-votekeeper-").expect("Failed to create temp dir");
    let temp_path = temp_dir.path().to_owned();

    if std::env::var("KEEP_TEMP").is_ok() {
        std::mem::forget(temp_dir);
    }

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
        "tests/votekeeper/votekeeperTest.qnt",
        &temp_path.to_string_lossy(),
        quint_seed,
    );

    for json_fixture in glob(&format!("{}/*.itf.json", temp_path.display()))
        .expect("Failed to read glob pattern")
        .flatten()
    {
        println!("🚀 Running trace {json_fixture:?}");

        let json = std::fs::read_to_string(&json_fixture).unwrap();
        let trace = itf::trace_from_str::<State>(&json).unwrap();

        let rng = StdRng::seed_from_u64(RANDOM_SEED);
        let public_keys = build_public_key_map(rng);
        let addresses = build_address_map(public_keys.iter());

        // build mapping from model addresses to real addresses
        let vote_keeper_runner = VoteKeeperRunner {
            addresses,
            public_keys,
        };

        trace.run_on(vote_keeper_runner).unwrap();
    }
}
