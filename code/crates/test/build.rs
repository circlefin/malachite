use std::io::Result;

fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.enable_type_names();
    config.extern_path(".malachite", "::malachite_proto");
    config.compile_protos(&["proto/test.proto"], &["proto", "../proto/proto"])?;

    Ok(())
}
