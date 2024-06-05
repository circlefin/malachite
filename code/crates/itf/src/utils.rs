use glob::glob;
use std::path::Path;

pub fn generate_traces(spec_rel_path: &str, gen_dir: &str, quint_seed: u64) {
    println!("🪄 Generating traces for {spec_rel_path:?}...");

    let spec_abs_path = format!(
        "{}/../../../specs/quint/{}",
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

    println!("🪄 Generated traces in {gen_dir:?}");
}
