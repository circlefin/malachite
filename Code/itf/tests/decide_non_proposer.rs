use malachite_itf::ItfState;

const FIXTURE_JSON: &str = include_str!("../fixtures/0DecideNonProposerTest.itf.json");

#[test]
fn parse_fixture() {
    let state = itf::trace_from_str::<ItfState>(FIXTURE_JSON).unwrap();
    dbg!(state);
}
