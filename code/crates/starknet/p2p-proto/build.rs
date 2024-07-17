fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = &[
        "./p2p-specs/p2p/proto/common.proto",
        "./p2p-specs/p2p/proto/header.proto",
        "./p2p-specs/p2p/proto/transaction.proto",
        "./p2p-specs/p2p/proto/consensus.proto",
    ];

    let mut config = prost_build::Config::new();
    config.enable_type_names();
    config.default_package_filename("p2p_specs");
    config.compile_protos(protos, &["./p2p-specs"])?;

    Ok(())
}
