use std::io::Result;

fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.enable_type_names();
    config.compile_protos(&["proto/test.proto"], &["proto"])?;

    Ok(())
}
