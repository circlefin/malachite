use std::io::Result;

fn main() -> Result<()> {
    let protos = &["proto/consensus.proto", "proto/sync.proto"];

    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }

    let mut config = prost_build::Config::new();
    config.enable_type_names();
    config.bytes(["."]);

    config.compile_protos(protos, &["proto"])?;

    Ok(())
}
