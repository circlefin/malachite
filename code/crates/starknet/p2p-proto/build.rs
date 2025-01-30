fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = &[
        "./proto/sync.proto",
        "./proto/p2p/proto/common.proto",
        "./proto/p2p/proto/transaction.proto",
        "./proto/p2p/proto/consensus/consensus.proto",
    ];

    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }

    let mut config = prost_build::Config::new();
    config.bytes(["."]);
    config.enable_type_names();
    config.default_package_filename("p2p");
    config.compile_protos(protos, &["./proto"])?;

    Ok(())
}
