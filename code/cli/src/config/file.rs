//! Node config.toml file configuration
//!
//! The node CLI reads configuration from the configuration file provided with the
//! `--config` parameter.
//!
//! The configuration is stored in the `Config` structure.
//! `confy` uses `serde` to read/write the configuration between the config file and the structure.
//!

use crate::config::args::Args;
use crate::config::serialization::{serde_base64, serde_debug_section_slice, serde_duration};
use crate::logging::DebugSection;
use confy::ConfyError;
use rand::prelude::StdRng;
use rand::rngs::OsRng;
use rand::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub config_file: PathBuf,

    /// Human-readable name for this node
    pub moniker: String,

    /// Genesis file path
    #[serde(default)]
    pub genesis_file: PathBuf,

    #[serde(with = "serde_debug_section_slice", default)]
    pub debug: Vec<DebugSection>,

    pub p2p: P2pConfig,

    pub consensus: Consensus,

    #[serde(default)]
    pub(crate) test: Test,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct P2pConfig {
    pub listen_addr: String,
    pub persistent_peers: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Consensus {
    #[serde(with = "serde_duration")]
    pub timeout_propose: Duration,
    #[serde(with = "serde_duration")]
    pub timeout_propose_delta: Duration,
    #[serde(with = "serde_duration")]
    pub timeout_prevote: Duration,
    #[serde(with = "serde_duration")]
    pub timeout_prevote_delta: Duration,
    #[serde(with = "serde_duration")]
    pub timeout_precommit: Duration,
    #[serde(with = "serde_duration")]
    pub timeout_precommit_delta: Duration,
    #[serde(with = "serde_duration")]
    pub timeout_commit: Duration,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Test {
    /// Validator index in Romain's test network
    #[serde(default)]
    pub index: usize,

    /// Base64-encoded private key
    #[serde(with = "serde_base64", default)]
    pub private_key: Vec<u8>,
    // Todo: change private_key to [u8;32] and move it to priv_validator_key.json
}

impl Default for Config {
    fn default() -> Self {
        let config_file = confy::get_configuration_file_path(env!("CARGO_PKG_NAME"), "config")
            .unwrap_or_else(|_| PathBuf::from("config.toml"));
        let moniker = format!("node-{}", OsRng.next_u64());
        let genesis_file = match config_file.parent() {
            None => "genesis.json".into(),
            Some(path) => path.join("genesis.json"),
        };
        let consensus = Consensus {
            timeout_propose: Duration::from_secs(3),
            timeout_propose_delta: Duration::from_millis(500),
            timeout_prevote: Duration::from_secs(1),
            timeout_prevote_delta: Duration::from_millis(500),
            timeout_precommit: Duration::from_secs(1),
            timeout_precommit_delta: Duration::from_millis(500),
            timeout_commit: Duration::from_secs(1),
        };
        let mut private_key = [0u8; 32];
        OsRng.fill_bytes(&mut private_key);
        let test = Test {
            index: 0,
            private_key: private_key.into(),
        };

        Self {
            config_file,
            moniker,
            genesis_file,
            debug: vec![],
            p2p: Default::default(),
            consensus,
            test,
        }
    }
}

// Todo: Merge this with malachite-node::Config
impl Config {
    /// Load configuration based on command-line arguments
    pub fn load(args: &Args) -> Result<Config, ConfyError> {
        // Figure out what to load.
        let config_file = match &args.config {
            Some(path) => {
                if path.exists() && path.is_file() {
                    path.clone()
                } else {
                    // If the file given in the command-line is missing, error out.
                    return Err(ConfyError::OpenConfigurationFileError(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "File not found",
                    )));
                }
            }
            // If no file was given, fall back to defaults.
            None => confy::get_configuration_file_path(env!("CARGO_PKG_NAME"), "config")?,
        };

        // Load file
        let mut cfg: Config = confy::load_path(config_file.clone())?;
        // Add the config file path
        cfg.config_file = config_file;

        // If the (optional) genesis_file was empty, fill it with the default value.
        match cfg.genesis_file.to_str() {
            Some(genesis_file) => {
                if genesis_file.is_empty() {
                    cfg.genesis_file = Config::default().genesis_file
                }
            }
            None => cfg.genesis_file = Config::default().genesis_file,
        }

        // If the (optional) private key was not provided in the file, generate a random set of bytes.
        // We will move this logic to priv_validator_key.json in the future.
        if cfg.test.private_key.is_empty() {
            cfg.test.private_key = Config::default().test.private_key;
        }
        if cfg.test.private_key.len() != 32 {
            return Err(ConfyError::GeneralLoadError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid private key length {}", cfg.test.private_key.len()),
            )));
        }

        // Merge command-line arguments into the configuration
        cfg.merge(args);

        // Apply testnet configuration
        if cfg.test.index > 0 {
            cfg.apply_testnet();
        }

        Ok(cfg)
    }

    /// Save configuration to file
    pub fn save(&self) {
        confy::store_path(&self.config_file, self).unwrap();
    }

    /// Merge the configuration with command-line parameters.
    fn merge(&mut self, args: &Args) {
        if let Some(genesis) = &args.genesis {
            self.genesis_file = genesis.clone();
        }

        if !args.private_key.is_empty() {
            self.test.private_key = args.private_key.clone();
        }

        if args.index != 0 {
            self.test.index = args.index;
        }

        if !args.debug.is_empty() {
            self.debug = args.debug.clone();
        }
    }

    fn apply_testnet(&mut self) {
        if self.test.index > 0 {
            let mut rng = StdRng::seed_from_u64(0x42);
            let mut bytes = [0u8; 32];

            for _ in 0..self.test.index {
                rng.fill_bytes(&mut bytes[..]);
            }
            self.test.private_key = bytes.to_vec();
            self.moniker = format!("node-{}", self.test.index);
        }
    }
}
