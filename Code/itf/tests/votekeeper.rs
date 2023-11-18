use std::path::PathBuf;

use malachite_itf::votekeeper::State;

use itf::Runner as ItfRunner;
use rstest::rstest;

mod votekeeper_utils;

use votekeeper_utils::runner::{vote_keeper_runner, VoteKeeperRunner};

#[rstest]
fn test_itf(
    #[files("tests/fixtures/votekeeper/*.itf.json")] json_fixture: PathBuf,
    mut vote_keeper_runner: VoteKeeperRunner,
) {
    println!("Parsing {json_fixture:?}");

    let json = std::fs::read_to_string(&json_fixture).unwrap();
    let trace = itf::trace_from_str::<State>(&json).unwrap();

    vote_keeper_runner
        .test(
            trace
                .states
                .into_iter()
                .map(|s| s.value)
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .unwrap();
}
