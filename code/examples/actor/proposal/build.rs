fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = &[
        "common.proto",
        "block.proto",
        "consensus.proto",
        "liveness.proto",
        "sync.proto",
    ];

    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }

    let fds = protox::compile(protos, ["./proto"])?;

    let mut config = prost_build::Config::new();
    config.bytes(["."]);
    config.enable_type_names();
    config.default_package_filename("test");
    config.compile_fds(fds)?;

    Ok(())
}
