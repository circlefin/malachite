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
use directories::BaseDirs;
use malachite_node::config::Config;
use malachite_test::{PrivateKey, ValidatorSet};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

const APP_FOLDER: &str = ".malachite";
const CONFIG_FILE: &str = "config.json";
const GENESIS_FILE: &str = "genesis.json";
const PRIV_VALIDATOR_KEY_FILE: &str = "priv_validator_key.json";

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

#[derive(Subcommand, Clone, Debug, Default, PartialEq)]
pub enum Commands {
    /// Initialize configuration
    Init,
    /// Start node
    #[default]
    Start,
}

impl Args {
    /// new returns a new instance of the configuration.
    pub fn new() -> Args {
        Args::parse()
    }

    /// get_home_dir returns the application home folder.
    /// Typically, `$HOME/.malachite`, dependent on the operating system.
    pub fn get_home_dir(&self) -> Result<PathBuf, ConfyError> {
        Ok(BaseDirs::new()
            .ok_or(ConfyError::BadConfigDirectory(
                "could not determine home directory path".to_string(),
            ))?
            .home_dir()
            .join(APP_FOLDER))
    }

    /// get_config_dir returns the configuration folder based on the home folder.
    pub fn get_config_dir(&self) -> Result<PathBuf, ConfyError> {
        Ok(self.get_home_dir()?.join("config"))
    }

    /// get_config_file_path returns the configuration file path based on the command-ine arguments
    /// and the configuration folder.
    pub fn get_config_file_path(&self) -> Result<PathBuf, ConfyError> {
        Ok(match &self.config {
            Some(path) => path.clone(),
            None => self.get_config_dir()?.join(CONFIG_FILE),
        })
    }

    /// get_genesis_file_path returns the genesis file path based on the command-line arguments and
    /// the configuration folder.
    pub fn get_genesis_file_path(&self) -> Result<PathBuf, ConfyError> {
        Ok(match &self.genesis {
            Some(path) => path.clone(),
            None => self.get_config_dir()?.join(GENESIS_FILE),
        })
    }

    /// get_priv_validator_key_file_path returns the private validator key file path based on the
    /// configuration folder.
    pub fn get_priv_validator_key_file_path(&self) -> Result<PathBuf, ConfyError> {
        Ok(self.get_config_dir()?.join(PRIV_VALIDATOR_KEY_FILE))
    }

    fn load_json_file<T>(&self, file: &PathBuf) -> Result<T, ConfyError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let mut content = String::new();
        File::open(file)
            .map_err(ConfyError::OpenConfigurationFileError)?
            .read_to_string(&mut content)
            .map_err(ConfyError::ReadConfigurationFileError)?;
        serde_json::from_str(&content).map_err(|e| ConfyError::GeneralLoadError(e.into()))
    }

    /// load_config returns a configuration compiled from the input parameters
    pub fn load_config(&self) -> Result<Config, ConfyError> {
        let config_file = self.get_config_file_path()?;
        let mut config: Config = confy::load_path(config_file)?;
        if let Some(index) = self.index {
            config.moniker = format!("test-{}", index);
        }
        Ok(config)
    }

    /// load_genesis returns the validator set from the genesis file
    pub fn load_genesis(&self) -> Result<ValidatorSet, ConfyError> {
        self.load_json_file(&self.get_genesis_file_path()?)
    }

    /// load_private_key returns the private key either from the command-line parameter or
    /// from the priv_validator_key.json file.
    pub fn load_private_key(&self) -> Result<PrivateKey, ConfyError> {
        if self.private_key.is_empty()
            || self.private_key == vec![0u8; 32]
            || self.private_key.len() < 32
        {
            self.load_json_file(&self.get_priv_validator_key_file_path()?)
        } else {
            let mut key: [u8; 32] = [0; 32];
            key.copy_from_slice(&self.private_key);
            Ok(PrivateKey::from(key))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_struct() {
        let args = Args::parse_from(&["test", "--debug", "ractor", "init"]);
        assert_eq!(args.debug, vec![DebugSection::Ractor]);
        assert_eq!(args.command, Commands::Init);

        let args = Args::parse_from(&["test", "start"]);
        assert_eq!(args.debug, vec![]);
        assert_eq!(args.command, Commands::Start);

        let args = Args::parse_from(&[
            "test",
            "--config",
            "myconfig.toml",
            "--genesis",
            "mygenesis.json",
            "--private-key",
            "c2VjcmV0",
            "init",
        ]);
        assert_eq!(args.config, Some(PathBuf::from("myconfig.toml")));
        assert_eq!(args.genesis, Some(PathBuf::from("mygenesis.json")));
        assert_eq!(args.private_key, b"secret");
        assert_eq!(args.index, None);
        assert!(args.get_home_dir().is_ok());
        assert!(args.get_config_dir().is_ok());
    }

    #[test]
    fn args_load_config() {
        let args = Args::parse_from(&["test", "--config", "../config.toml", "start"]);
        let config = args.load_config().unwrap();
        assert_eq!(config.moniker, "malachite");
    }

    #[test]
    fn args_load_genesis() {
        let args = Args::parse_from(&["test", "--genesis", "../genesis.json", "start"]);
        assert!(args.load_genesis().is_err());
    }

    #[test]
    fn args_private_key() {
        let args = Args::parse_from(&["test", "start"]);
        assert!(args.load_private_key().is_err());

        let args = Args::parse_from(&["test", "--private-key", "c2VjcmV0", "start"]);
        assert!(args.load_private_key().is_err());

        let args = Args::parse_from(&[
            "test",
            "--private-key",
            "c2VjcmV0c2VjcmV0c2VjcmV0c2VjcmV0c2VjcmV0MDA=",
            "start",
        ]);
        let pk = args.load_private_key().unwrap();

        assert_eq!(pk.inner().as_bytes(), b"secretsecretsecretsecretsecret00");
    }
}
