#[path = "votekeeper/runner.rs"]
pub mod runner;
#[path = "votekeeper/utils.rs"]
pub mod utils;

use glob::glob;
use rand::rngs::StdRng;
use rand::SeedableRng;

use malachite_itf::votekeeper::State;
use malachite_test::{Address, PrivateKey};

use runner::VoteKeeperRunner;
use utils::ADDRESSES;

const RANDOM_SEED: u64 = 0x42;

#[test]
fn test_itf() {
    for json_fixture in glob("tests/fixtures/votekeeper/*.itf.json")
        .expect("Failed to read glob pattern")
        .flatten()
    {
        println!("Parsing {json_fixture:?}");

        let json = std::fs::read_to_string(&json_fixture).unwrap();
        let trace = itf::trace_from_str::<State>(&json).unwrap();

        let mut rng = StdRng::seed_from_u64(RANDOM_SEED);

        // build mapping from model addresses to real addresses
        let vote_keeper_runner = VoteKeeperRunner {
            address_map: ADDRESSES
                .iter()
                .map(|&name| {
                    let pk = PrivateKey::generate(&mut rng).public_key();
                    (name.into(), Address::from_public_key(&pk))
                })
                .collect(),
        };

        trace.run_on(vote_keeper_runner).unwrap();
    }
}
