//! File save functions

use std::fs;
use std::path::Path;

use malachitebft_app::node::Node;

use crate::error::Error;

/// Save configuration to file
pub fn save_config<N: Node>(config_file: &Path, config: &N::Config) -> Result<(), Error> {
    save(
        config_file,
        &toml::to_string_pretty(config).map_err(|e| Error::ToJSON(e.to_string()))?,
    )
}

/// Save genesis to file
pub fn save_genesis<N: Node>(
    _node: &N,
    genesis_file: &Path,
    genesis: &N::Genesis,
) -> Result<(), Error> {
    save(
        genesis_file,
        &serde_json::to_string_pretty(genesis).map_err(|e| Error::ToJSON(e.to_string()))?,
    )
}

/// Save private_key validator key to file
pub fn save_priv_validator_key<N: Node>(
    _node: &N,
    priv_validator_key_file: &Path,
    priv_validator_key: &N::PrivateKeyFile,
) -> Result<(), Error> {
    save(
        priv_validator_key_file,
        &serde_json::to_string_pretty(priv_validator_key)
            .map_err(|e| Error::ToJSON(e.to_string()))?,
    )
}

fn save(path: &Path, data: &str) -> Result<(), Error> {
    use std::io::Write;

    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir).map_err(|_| Error::ParentDir(parent_dir.to_path_buf()))?;
    }

    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|_| Error::OpenFile(path.to_path_buf()))?;

    f.write_all(data.as_bytes())
        .map_err(|_| Error::WriteFile(path.to_path_buf()))?;

    Ok(())
}
