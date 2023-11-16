use malachite_itf::consensus::State;
use rstest::rstest;
use std::path::PathBuf;

#[rstest]
fn test_itf(#[files("tests/fixtures/consensus/*.json")] json_fixture: PathBuf) {
    println!("Parsing {json_fixture:?}");

    let json = std::fs::read_to_string(&json_fixture).unwrap();
    let state = itf::trace_from_str::<State>(&json).unwrap();

    dbg!(state);
}
