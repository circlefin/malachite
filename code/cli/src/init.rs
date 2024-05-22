//! Init command

use std::fs::{self};
use std::path::PathBuf;

use color_eyre::eyre::{eyre, Result};
use tracing::debug;

use malachite_node::config::Config;
use malachite_test::PrivateKey;
use malachite_test::ValidatorSet as Genesis;

use crate::example::{generate_config, generate_genesis, generate_private_key};

/// Execute the init command
pub fn run(
    config_file: PathBuf,
    genesis_file: PathBuf,
    priv_validator_key_file: PathBuf,
    index: usize,
) -> Result<()> {
    // Save default configuration
    if !config_file.exists() {
        debug!("Saving configuration to {:?}.", config_file);
        save_config(&config_file, &generate_config(index))?;
    }

    // Save default genesis
    if !genesis_file.exists() {
        debug!("Saving test genesis to {:?}.", genesis_file);
        save_genesis(&genesis_file, &generate_genesis())?;
    }

    // Save default priv_validator_key
    if !priv_validator_key_file.exists() {
        debug!("Saving private key to {:?}.", priv_validator_key_file);
        save_priv_validator_key(&priv_validator_key_file, &generate_private_key(index))?;
    }

    Ok(())
}

/// Save configuration to file
pub fn save_config(config_file: &PathBuf, config: &Config) -> Result<()> {
    save(config_file, &toml::to_string_pretty(config)?)
}

/// Save genesis to file
pub fn save_genesis(genesis_file: &PathBuf, genesis: &Genesis) -> Result<()> {
    save(genesis_file, &serde_json::to_string_pretty(genesis)?)
}

/// Save private_key validator key to file
pub fn save_priv_validator_key(
    priv_validator_key_file: &PathBuf,
    private_key: &PrivateKey,
) -> Result<()> {
    save(
        priv_validator_key_file,
        &serde_json::to_string_pretty(private_key)?,
    )
}

fn save(path: &PathBuf, data: &str) -> Result<()> {
    use std::io::Write;

    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir).map_err(|e| {
            eyre!(
                "Failed to create parent directory {}: {e:?}",
                parent_dir.display()
            )
        })?;
    }

    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|e| {
            eyre!(
                "Failed to crate configuration file at {}: {e:?}",
                path.display()
            )
        })?;

    f.write_all(data.as_bytes())
        .map_err(|e| eyre!("Failed to write configuration to {}: {e:?}", path.display()))?;

    Ok(())
}
