use malachite_itf::consensus::State;

#[test]
fn parse_fixtures() {
    let folder = format!("{}/tests/fixtures/consensus", env!("CARGO_MANIFEST_DIR"));

    let fixtures = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().map_or(false, |ext| ext == "json"))
        .collect::<Vec<_>>();

    for fixture in fixtures {
        println!("Parsing '{}'", fixture.display());

        let json = std::fs::read_to_string(&fixture).unwrap();
        let state = itf::trace_from_str::<State>(&json).unwrap();

        dbg!(state);
    }
}
