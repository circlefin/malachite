use std::io::Result;

fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.enable_type_names();
    config.extern_path(".malachite.common", "::malachite_common::proto");
    config.compile_protos(&["proto/mock.proto"], &["proto", "../common/proto"])?;

    Ok(())
}
