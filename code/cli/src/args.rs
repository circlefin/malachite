//! Node command-line interface configuration
//!
//! The node CLI reads configuration from the configuration files found in the directory
//! provided with the `--home` global parameter.
//!
//! The command-line parameters are stored in the `Args` structure.
//! `clap` parses the command-line parameters into this structure.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use color_eyre::eyre::{eyre, Context, Result};
use directories::BaseDirs;
use tracing::info;

use malachite_node::config::Config;
use malachite_test::{PrivateKey, ValidatorSet};

use crate::logging::DebugSection;
use crate::priv_key::PrivValidatorKey;

const APP_FOLDER: &str = ".malachite";
const CONFIG_FILE: &str = "config.toml";
const GENESIS_FILE: &str = "genesis.json";
const PRIV_VALIDATOR_KEY_FILE: &str = "priv_validator_key.json";

#[derive(Parser, Clone, Debug, Default)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Home directory for Malachite (default: `~/.malachite`)
    #[arg(long, global = true, value_name = "HOME_DIR")]
    pub home: Option<PathBuf>,

    #[clap(
        long,
        global = true,
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
    /// Start node
    #[default]
    Start,

    /// Initialize configuration
    Init,

    /// Generate testnet configuration
    Testnet(TestnetArgs),
}

#[derive(Parser, Debug, Clone, PartialEq)]
pub struct TestnetArgs {
    /// Number of validator nodes in the testnet
    #[clap(short, long)]
    pub nodes: usize,

    /// Generate deterministic private keys for reproducibility
    #[clap(short, long)]
    pub deterministic: bool,
}

impl Args {
    /// new returns a new instance of the configuration.
    pub fn new() -> Args {
        Args::parse()
    }

    /// get_home_dir returns the application home folder.
    /// Typically, `$HOME/.malachite`, dependent on the operating system.
    pub fn get_home_dir(&self) -> Result<PathBuf> {
        match self.home {
            Some(ref path) => Ok(path.clone()),
            None => Ok(BaseDirs::new()
                .ok_or_else(|| eyre!("could not determine home directory path"))?
                .home_dir()
                .join(APP_FOLDER)),
        }
    }

    /// get_config_dir returns the configuration folder based on the home folder.
    pub fn get_config_dir(&self) -> Result<PathBuf> {
        Ok(self.get_home_dir()?.join("config"))
    }

    /// get_config_file_path returns the configuration file path based on the command-ine arguments
    /// and the configuration folder.
    pub fn get_config_file_path(&self) -> Result<PathBuf> {
        Ok(self.get_config_dir()?.join(CONFIG_FILE))
    }

    /// get_genesis_file_path returns the genesis file path based on the command-line arguments and
    /// the configuration folder.
    pub fn get_genesis_file_path(&self) -> Result<PathBuf> {
        Ok(self.get_config_dir()?.join(GENESIS_FILE))
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
        load_toml_file(&config_file)
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
        let priv_key_file = self.get_priv_validator_key_file_path()?;
        info!("Loading private key from {:?}", priv_key_file.display());
        let priv_validator_key: PrivValidatorKey = load_json_file(&priv_key_file)?;
        Ok(priv_validator_key.private_key)
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
}
