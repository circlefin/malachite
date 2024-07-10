use std::io::Result;

#[cfg(feature = "proto")]
fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.enable_type_names();
    config.compile_protos(&["proto/malachite.common.proto"], &["proto"])?;

    Ok(())
}

#[cfg(not(feature = "proto"))]
fn main() -> Result<()> {
    Ok(())
}
