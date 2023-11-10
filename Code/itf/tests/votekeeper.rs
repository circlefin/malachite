use malachite_itf::votekeeper::State;

const FIXTURES: &[&str] = &["votekeeper.itf.json"];

#[test]
fn parse_fixtures() {
    for fixture in FIXTURES {
        println!("Parsing '{fixture}'");

        let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), fixture);

        let json = std::fs::read_to_string(&path).unwrap();
        let trace = itf::trace_from_str::<State>(&json).unwrap();

        dbg!(trace);
    }
}
