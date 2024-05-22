//! Init command

use std::fs;
use std::path::Path;

use color_eyre::eyre::{eyre, Context, Result};
use tracing::{info, warn};

use malachite_node::config::Config;
use malachite_test::PrivateKey;
use malachite_test::ValidatorSet as Genesis;

use crate::example::{generate_config, generate_genesis, generate_private_key};

/// Execute the init command
pub fn run(
    config_file: &Path,
    genesis_file: &Path,
    priv_validator_key_file: &Path,
    index: usize,
) -> Result<()> {
    // Save default configuration
    if config_file.exists() {
        warn!(
            "Configuration file already exists at {:?}, skipping.",
            config_file.display()
        )
    } else {
        info!("Saving configuration to {:?}.", config_file);
        save_config(config_file, &generate_config(index))?;
    }

    // Save default genesis
    if genesis_file.exists() {
        warn!(
            "Genesis file already exists at {:?}, skipping.",
            genesis_file.display()
        )
    } else {
        info!("Saving test genesis to {:?}.", genesis_file);
        save_genesis(genesis_file, &generate_genesis())?;
    }

    // Save default priv_validator_key
    if priv_validator_key_file.exists() {
        warn!(
            "Private key file already exists at {:?}, skipping.",
            priv_validator_key_file.display()
        )
    } else {
        info!("Saving private key to {:?}.", priv_validator_key_file);
        save_priv_validator_key(priv_validator_key_file, &generate_private_key(index))?;
    }

    Ok(())
}

/// Save configuration to file
pub fn save_config(config_file: &Path, config: &Config) -> Result<()> {
    save(config_file, &toml::to_string_pretty(config)?)
}

/// Save genesis to file
pub fn save_genesis(genesis_file: &Path, genesis: &Genesis) -> Result<()> {
    save(genesis_file, &serde_json::to_string_pretty(genesis)?)
}

/// Save private_key validator key to file
pub fn save_priv_validator_key(
    priv_validator_key_file: &Path,
    private_key: &PrivateKey,
) -> Result<()> {
    save(
        priv_validator_key_file,
        &serde_json::to_string_pretty(private_key)?,
    )
}

fn save(path: &Path, data: &str) -> Result<()> {
    use std::io::Write;

    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir).wrap_err_with(|| {
            eyre!(
                "Failed to create parent directory {:?}",
                parent_dir.display()
            )
        })?;
    }

    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .wrap_err_with(|| eyre!("Failed to crate configuration file at {:?}", path.display()))?;

    f.write_all(data.as_bytes())
        .wrap_err_with(|| eyre!("Failed to write configuration to {:?}", path.display()))?;

    Ok(())
}
