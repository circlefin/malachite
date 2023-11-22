use std::path::PathBuf;

use malachite_itf::votekeeper::State;

use rstest::rstest;

#[path = "votekeeper/runner.rs"]
pub mod runner;
#[path = "votekeeper/utils.rs"]
pub mod utils;

use runner::{vote_keeper_runner, VoteKeeperRunner};

#[rstest]
fn test_itf(
    #[files("tests/fixtures/votekeeper/*.itf.json")] json_fixture: PathBuf,
    vote_keeper_runner: VoteKeeperRunner,
) {
    println!("Parsing {json_fixture:?}");

    let json = std::fs::read_to_string(&json_fixture).unwrap();
    let trace = itf::trace_from_str::<State>(&json).unwrap();
    trace.run_on(vote_keeper_runner).unwrap();
}
