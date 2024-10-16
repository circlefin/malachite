//! Init command

use std::fs;
use std::path::Path;

use clap::Parser;
use color_eyre::eyre::{eyre, Context, Result};
use tracing::{info, warn};

use malachite_node::config::{App, Config, LogFormat, LogLevel, TransportProtocol};
use malachite_node::Node;
use malachite_starknet_app::node::StarknetNode;

use crate::cmd::testnet::{
    generate_config, generate_genesis, generate_private_keys, RuntimeFlavour,
};

#[derive(Parser, Debug, Clone, Default, PartialEq)]
pub struct InitCmd {
    /// The name of the application to run
    #[clap(long, default_value_t = App::default())]
    pub app: App,

    /// Overwrite existing configuration files
    #[clap(long)]
    pub overwrite: bool,

    /// Enable peer discovery.
    /// If enabled, the node will attempt to discover other nodes in the network
    #[clap(long, default_value = "true")]
    pub enable_discovery: bool,
}

impl InitCmd {
    /// Execute the init command
    pub fn run(
        &self,
        config_file: &Path,
        genesis_file: &Path,
        priv_validator_key_file: &Path,
        log_level: LogLevel,
        log_format: LogFormat,
    ) -> Result<()> {
        let node = match self.app {
            App::Starknet => StarknetNode,
        };

        // Save default configuration
        if config_file.exists() && !self.overwrite {
            warn!(
                "Configuration file already exists at {:?}, skipping",
                config_file.display()
            )
        } else {
            info!(file = ?config_file, "Saving configuration");
            save_config(
                config_file,
                &generate_config(
                    self.app,
                    0,
                    1,
                    RuntimeFlavour::SingleThreaded,
                    self.enable_discovery,
                    TransportProtocol::Tcp,
                    log_level,
                    log_format,
                ),
            )?;
        }

        // Save default genesis
        if genesis_file.exists() && !self.overwrite {
            warn!(
                "Genesis file already exists at {:?}, skipping",
                genesis_file.display()
            )
        } else {
            let private_keys = generate_private_keys(&node, 1, true);
            let public_keys = private_keys.iter().map(|pk| pk.public_key()).collect();
            let genesis = generate_genesis(&node, public_keys, true);
            info!(file = ?genesis_file, "Saving test genesis");
            save_genesis(&node, genesis_file, &genesis)?;
        }

        // Save default priv_validator_key
        if priv_validator_key_file.exists() && !self.overwrite {
            warn!(
                "Private key file already exists at {:?}, skipping",
                priv_validator_key_file.display()
            )
        } else {
            info!(file = ?priv_validator_key_file, "Saving private key");
            let private_keys = generate_private_keys(&node, 1, false);
            let priv_validator_key = node.make_private_key_file(private_keys[0]);
            save_priv_validator_key(&node, priv_validator_key_file, &priv_validator_key)?;
        }

        Ok(())
    }
}

/// Save configuration to file
pub fn save_config(config_file: &Path, config: &Config) -> Result<()> {
    save(config_file, &toml::to_string_pretty(config)?)
}

/// Save genesis to file
pub fn save_genesis<N: Node>(_node: &N, genesis_file: &Path, genesis: &N::Genesis) -> Result<()> {
    save(genesis_file, &serde_json::to_string_pretty(genesis)?)
}

/// Save private_key validator key to file
pub fn save_priv_validator_key<N: Node>(
    _node: &N,
    priv_validator_key_file: &Path,
    priv_validator_key: &N::PrivateKeyFile,
) -> Result<()> {
    save(
        priv_validator_key_file,
        &serde_json::to_string_pretty(priv_validator_key)?,
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
