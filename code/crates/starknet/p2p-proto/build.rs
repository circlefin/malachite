fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = &[
        "./proto/p2p/proto/common.proto",
        "./proto/p2p/proto/header.proto",
        "./proto/p2p/proto/transaction.proto",
        "./proto/p2p/proto/consensus.proto",
        "./proto/p2p/proto/streaming.proto",
    ];

    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }

    let mut config = prost_build::Config::new();
    config.bytes(["."]);
    config.enable_type_names();
    config.default_package_filename("p2p_specs");
    config.compile_protos(protos, &["./proto"])?;

    Ok(())
}
