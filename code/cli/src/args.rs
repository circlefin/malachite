//! Node command-line interface configuration
//!
//! The node CLI reads configuration from the configuration file provided with the
//! `--config` parameter. Some configuration parameters can be overridden on the command-line.
//!
//! The command-line parameters are stored in the `Args` structure.
//! `clap` parses the command-line parameters into this structure.
//!

use std::path::{Path, PathBuf};

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use clap::{Parser, Subcommand};
use color_eyre::eyre::{eyre, Context, Result};
use directories::BaseDirs;
use malachite_node::config::Config;
use malachite_test::{PrivateKey, ValidatorSet};
use tracing::info;

use crate::logging::DebugSection;

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
    #[clap(
        long,
        default_value = "",
        hide_default_value = true,
        value_name = "BASE64_STRING",
        env = "PRIVATE_KEY",
        value_parser = |s: &str| BASE64_STANDARD.decode(s)
    )]
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
    pub fn get_home_dir(&self) -> Result<PathBuf> {
        Ok(BaseDirs::new()
            .ok_or_else(|| eyre!("could not determine home directory path"))?
            .home_dir()
            .join(APP_FOLDER))
    }

    /// get_config_dir returns the configuration folder based on the home folder.
    pub fn get_config_dir(&self) -> Result<PathBuf> {
        Ok(self.get_home_dir()?.join("config"))
    }

    /// get_config_file_path returns the configuration file path based on the command-ine arguments
    /// and the configuration folder.
    pub fn get_config_file_path(&self) -> Result<PathBuf> {
        Ok(match &self.config {
            Some(path) => path.clone(),
            None => self.get_config_dir()?.join(CONFIG_FILE),
        })
    }

    /// get_genesis_file_path returns the genesis file path based on the command-line arguments and
    /// the configuration folder.
    pub fn get_genesis_file_path(&self) -> Result<PathBuf> {
        Ok(match &self.genesis {
            Some(path) => path.clone(),
            None => self.get_config_dir()?.join(GENESIS_FILE),
        })
    }

    /// get_priv_validator_key_file_path returns the private validator key file path based on the
    /// configuration folder.
    pub fn get_priv_validator_key_file_path(&self) -> Result<PathBuf> {
        Ok(self.get_config_dir()?.join(PRIV_VALIDATOR_KEY_FILE))
    }

    /// load_config returns a configuration compiled from the input parameters
    pub fn load_config(&self) -> Result<Config> {
        let config_file = self.get_config_file_path()?;
        info!("Loading configuration from {:?}", config_file.display());
        let mut config: Config = load_toml_file(&config_file)?;
        if let Some(index) = self.index {
            config.moniker = format!("test-{}", index);
        }
        Ok(config)
    }

    /// load_genesis returns the validator set from the genesis file
    pub fn load_genesis(&self) -> Result<ValidatorSet> {
        let genesis_file = self.get_genesis_file_path()?;
        info!("Loading genesis from {:?}", genesis_file.display());
        load_json_file(&genesis_file)
    }

    /// load_private_key returns the private key either from the command-line parameter or
    /// from the priv_validator_key.json file.
    pub fn load_private_key(&self) -> Result<PrivateKey> {
        if self.private_key.is_empty()
            || self.private_key == vec![0u8; 32]
            || self.private_key.len() < 32
        {
            let priv_key_file = self.get_priv_validator_key_file_path()?;
            info!("Loading private key from {:?}", priv_key_file.display());
            load_json_file(&priv_key_file)
        } else {
            let mut key: [u8; 32] = [0; 32];
            key.copy_from_slice(&self.private_key);
            Ok(PrivateKey::from(key))
        }
    }
}

fn load_json_file<T>(file: &Path) -> Result<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let content = std::fs::read_to_string(file)
        .wrap_err_with(|| eyre!("Failed to read configuration file at {}", file.display()))?;

    serde_json::from_str(&content)
        .wrap_err_with(|| eyre!("Failed to load configuration at {}", file.display(),))
}

fn load_toml_file<T>(file: &Path) -> Result<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let content = std::fs::read_to_string(file)
        .wrap_err_with(|| eyre!("Failed to read configuration file at {}", file.display()))?;

    toml::from_str(&content)
        .wrap_err_with(|| eyre!("Failed to load configuration at {}", file.display(),))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_struct() {
        let args = Args::parse_from(["test", "--debug", "ractor", "init"]);
        assert_eq!(args.debug, vec![DebugSection::Ractor]);
        assert_eq!(args.command, Commands::Init);

        let args = Args::parse_from(["test", "start"]);
        assert_eq!(args.debug, vec![]);
        assert_eq!(args.command, Commands::Start);

        let args = Args::parse_from([
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
    fn args_methods() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        #[derive(serde::Deserialize)]
        struct TestStruct {}

        let args = Args::parse_from(["test", "start"]);
        assert!(args.get_config_file_path().is_ok());
        assert!(args.get_genesis_file_path().is_ok());
        assert!(load_json_file::<TestStruct>(&PathBuf::from("nonexistent.json")).is_err());

        let tmpfile = NamedTempFile::new().unwrap();
        let mut file = tmpfile.as_file();
        writeln!(file, "{{}}").unwrap();
        assert!(load_json_file::<TestStruct>(&PathBuf::from(tmpfile.path())).is_ok());
    }

    #[test]
    fn args_load_config() {
        let args = Args::parse_from(["test", "--config", "../config.toml", "start"]);
        let config = args.load_config().unwrap();
        assert_eq!(config.moniker, "malachite");

        // Testnet configuration
        let args = Args::parse_from([
            "test",
            "--config",
            "../config.toml",
            "--index",
            "2",
            "start",
        ]);
        let config = args.load_config().unwrap();
        assert_eq!(config.moniker, "test-2");
    }

    #[test]
    fn args_load_genesis() {
        let args = Args::parse_from(["test", "--genesis", "../genesis.json", "start"]);
        assert!(args.load_genesis().is_err());
    }

    #[test]
    fn args_private_key() {
        let args = Args::parse_from(["test", "start"]);
        if !args.get_priv_validator_key_file_path().unwrap().exists() {
            assert!(args.load_private_key().is_err());
            assert!(args.private_key.is_empty());
        }

        let args = Args::parse_from(["test", "--private-key", "c2VjcmV0", "start"]);
        if !args.get_priv_validator_key_file_path().unwrap().exists() {
            assert!(args.load_private_key().is_err());
        }

        let args = Args::parse_from([
            "test",
            "--private-key",
            "c2VjcmV0c2VjcmV0c2VjcmV0c2VjcmV0c2VjcmV0MDA=",
            "start",
        ]);
        let pk = args.load_private_key().unwrap();

        assert_eq!(pk.inner().as_bytes(), b"secretsecretsecretsecretsecret00");
    }
}
