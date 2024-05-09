use std::io::Result;

fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.enable_type_names();
    dbg!(&config);
    config.compile_protos(&["src/malachite.proto"], &["src/"])?;

    Ok(())
}
