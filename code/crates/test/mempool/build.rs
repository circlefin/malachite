fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = &["proto/malachite.mempool.proto"];

    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }

    let fds = protox::compile(protos, ["proto"])?;

    let mut config = prost_build::Config::new();
    config.enable_type_names();
    config.compile_fds(fds)?;

    Ok(())
}
