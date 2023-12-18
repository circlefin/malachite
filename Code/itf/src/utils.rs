use glob::glob;
use std::path::Path;

// TODO(rano): simplify this function once quint is fixed
pub fn generate_traces(spec_rel_path: &str, gen_dir: &str, quint_seed: u64) {
    println!("🪄 Generating traces for {spec_rel_path:?}...");

    let spec_abs_path = format!(
        "{}/../../Specs/Quint/{}",
        env!("CARGO_MANIFEST_DIR"),
        spec_rel_path
    );

    let spec_path = Path::new(&spec_abs_path);

    std::process::Command::new("quint")
        .arg("test")
        .arg("--output")
        .arg(format!("{}/{{}}.itf.json", gen_dir))
        .arg("--seed")
        .arg(quint_seed.to_string())
        .arg(spec_path)
        .current_dir(spec_path.parent().unwrap())
        .output()
        .expect("Failed to run quint test");

    // Remove traces from imported modules
    for redundant_itf in glob(&format!(
        "{}/*{}::*.*",
        gen_dir,
        spec_path.file_stem().unwrap().to_str().unwrap()
    ))
    .expect("Failed to read glob pattern")
    .flatten()
    {
        std::fs::remove_file(&redundant_itf).unwrap();
    }

    // Rerun quint per tests
    // https://github.com/informalsystems/quint/issues/1263
    for itf_json in glob(&format!("{}/*.itf.json", gen_dir,))
        .expect("Failed to read glob pattern")
        .flatten()
    {
        std::fs::remove_file(&itf_json).unwrap();

        std::process::Command::new("quint")
            .arg("test")
            .arg("--output")
            .arg(format!(
                "{}/{}_{{}}.itf.json",
                gen_dir,
                spec_path.file_stem().unwrap().to_str().unwrap()
            ))
            .arg("--match")
            .arg(
                itf_json
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .strip_suffix(".itf.json")
                    .unwrap(),
            )
            .arg("--seed")
            .arg(quint_seed.to_string())
            .arg(spec_path)
            .current_dir(spec_path.parent().unwrap())
            .output()
            .expect("Failed to run quint test");
    }

    // Remove duplicate states
    // https://github.com/informalsystems/quint/issues/1252
    for itf_json in glob(&format!("{}/*.itf.json", gen_dir,))
        .expect("Failed to read glob pattern")
        .flatten()
    {
        let mut json: serde_json::Value =
            serde_json::from_reader(std::fs::File::open(&itf_json).unwrap()).unwrap();

        let states = json["states"].as_array_mut().unwrap();
        states.retain(|state| {
            let index = state["#meta"]["index"].as_u64().unwrap();
            index % 2 == 0
        });
        states.iter_mut().enumerate().for_each(|(i, state)| {
            state["#meta"]["index"] = serde_json::Value::from(i as u64);
        });

        let mut json_file = std::fs::File::create(&itf_json).unwrap();
        serde_json::to_writer_pretty(&mut json_file, &json).unwrap();
    }
}
