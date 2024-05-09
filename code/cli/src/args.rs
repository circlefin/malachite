//! Node command-line interface configuration
//!
//! The node CLI reads configuration from the configuration file provided with the
//! `--config` parameter. Some configuration parameters can be overridden on the command-line.
//!
//! The command-line parameters are stored in the `Args` structure.
//! `clap` parses the command-line parameters into this structure.
//!

use crate::logging::DebugSection;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use clap::{Parser, Subcommand};
use confy::ConfyError;
use malachite_node::config::Config;
use malachite_test::ValidatorSet as Genesis;
use malachite_test::{PrivateKey, Validator, ValidatorSet};
use rand::prelude::StdRng;
use rand::SeedableRng;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Subcommand, Clone, Debug, Default)]
pub enum Commands {
    /// Initialize configuration
    Init,
    /// Start node
    #[default]
    Start,
}

#[derive(Parser, Clone, Debug, Default)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Config file path
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Genesis file path
    #[arg(short, long, value_name = "FILE")]
    pub genesis: Option<PathBuf>,

    /// Base64-encoded private key
    #[clap(long, default_value="", hide_default_value=true, value_name = "BASE64_STRING", env="PRIVATE_KEY", value_parser = |s: &str| {BASE64_STANDARD.decode(s)})]
    pub private_key: std::vec::Vec<u8>, // Keep the fully qualified path for Vec<u8> or else clap will not be able to parse it: https://github.com/clap-rs/clap/issues/4481.

    /// Validator index in Romain's test network
    #[clap(short, long, value_name = "INDEX", env = "INDEX")]
    pub index: Option<usize>,

    #[clap(
        short,
        long = "debug",
        help = "Enable debug output for the given comma-separated sections",
        value_enum,
        value_delimiter = ','
    )]
    pub debug: Vec<DebugSection>,

    #[command(subcommand)]
    pub command: Commands,
}

impl Args {
    /// new returns a new instance of the configuration.
    pub fn new() -> Args {
        Args::parse()
    }

    pub fn get_config_file_path(&self) -> Result<PathBuf, ConfyError> {
        match &self.config {
            Some(path) => Ok(path.clone()),
            None => confy::get_configuration_file_path(env!("CARGO_PKG_NAME"), "config"),
        }
    }

    pub fn get_genesis_file_path(&self) -> Result<PathBuf, ConfyError> {
        match &self.genesis {
            Some(path) => Ok(path.clone()),
            None => match self.get_config_file_path() {
                Ok(path) => match path.parent() {
                    None => confy::get_configuration_file_path(env!("CARGO_PKG_NAME"), "genesis"),
                    Some(parent) => Ok(parent.join("genesis.json")),
                },
                Err(_) => confy::get_configuration_file_path(env!("CARGO_PKG_NAME"), "genesis"),
            },
        }
    }

    pub fn get_priv_validator_key_file_path(&self) -> Result<PathBuf, ConfyError> {
        match &self.get_config_file_path() {
            Ok(path) => match path.parent() {
                None => {
                    confy::get_configuration_file_path(env!("CARGO_PKG_NAME"), "priv_validator_key")
                }
                Some(parent) => Ok(parent.join("priv_validator_key.json")),
            },
            Err(_) => {
                confy::get_configuration_file_path(env!("CARGO_PKG_NAME"), "priv_validator_key")
            }
        }
    }
}

impl TryFrom<Args> for Config {
    type Error = ConfyError;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        let config_file = args.get_config_file_path()?;
        confy::load_path(config_file)
    }
}

impl TryFrom<Args> for ValidatorSet {
    type Error = ConfyError;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        let genesis_file = args.get_genesis_file_path()?;
        let file = File::open(genesis_file).map_err(ConfyError::OpenConfigurationFileError)?;
        let reader = BufReader::new(file);
        Ok(serde_json::from_reader(reader).unwrap())
    }
}

impl TryFrom<Args> for PrivateKey {
    type Error = ConfyError;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        if args.private_key.is_empty()
            || args.private_key == vec![0u8; 32]
            || args.private_key.len() != 32
        {
            let priv_validator_key_file = args.get_priv_validator_key_file_path()?;
            let file = File::open(priv_validator_key_file)
                .map_err(ConfyError::OpenConfigurationFileError)?;
            let reader = BufReader::new(file);
            Ok(serde_json::from_reader(reader).unwrap())
        } else {
            Err(ConfyError::OpenConfigurationFileError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No valid private key found",
            )))
        }
    }
}

/// Save configuration to file
pub fn save_config(config_file: &PathBuf, cfg: &Config) {
    confy::store_path(config_file, cfg).unwrap();
}

/// Save genesis to file
pub fn save_genesis(genesis_file: &PathBuf, genesis: &Genesis) {
    let file = File::create(genesis_file).unwrap();
    serde_json::to_writer_pretty(file, genesis).unwrap();
}

/// Save private_key validator key to file
pub fn save_priv_validator_key(priv_validator_key_file: &PathBuf, private_key: &PrivateKey) {
    let file = File::create(priv_validator_key_file).unwrap();
    serde_json::to_writer_pretty(file, private_key).unwrap();
}

/// Generate a test genesis configuration
pub fn generate_test_genesis() -> Genesis {
    let voting_power = vec![11, 10, 10];

    let mut rng = StdRng::seed_from_u64(0x42);
    let mut validators = Vec::with_capacity(voting_power.len());

    for vp in voting_power {
        validators.push(Validator::new(
            PrivateKey::generate(&mut rng).public_key(),
            vp,
        ));
    }

    Genesis { validators }
}

pub fn generate_test_private_key(index: usize) -> PrivateKey {
    let mut rng = StdRng::seed_from_u64(0x42);
    for _ in 0..index {
        let _ = PrivateKey::generate(&mut rng);
    }
    PrivateKey::generate(&mut rng)
}
