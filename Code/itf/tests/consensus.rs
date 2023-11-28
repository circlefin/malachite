use glob::glob;

use malachite_itf::consensus::State;

#[test]
fn test_itf() {
    for json_fixture in glob("tests/fixtures/consensus/*.json")
        .expect("Failed to read glob pattern")
        .flatten()
    {
        println!("Parsing {json_fixture:?}");

        let json = std::fs::read_to_string(&json_fixture).unwrap();
        let state = itf::trace_from_str::<State>(&json).unwrap();

        dbg!(state);
    }
}
