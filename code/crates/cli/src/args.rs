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

use malachite_node::config::{Config, LogFormat, LogLevel};
use malachite_test::{PrivateKey, ValidatorSet};

use crate::cmd::init::InitCmd;
use crate::cmd::keys::KeysCmd;
use crate::cmd::start::StartCmd;
use crate::cmd::testnet::TestnetCmd;
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

    /// Log level (default: `malachite=debug`)
    #[arg(long, global = true, value_name = "LOG_LEVEL")]
    pub log_level: Option<LogLevel>,

    /// Log format (default: `plaintext`)
    #[arg(long, global = true, value_name = "LOG_FORMAT")]
    pub log_format: Option<LogFormat>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Start node
    Start(StartCmd),

    /// Initialize configuration
    Init(InitCmd),

    /// Manage keys
    #[command(subcommand)]
    Keys(KeysCmd),

    /// Generate testnet configuration
    Testnet(TestnetCmd),
}

impl Default for Commands {
    fn default() -> Self {
        Commands::Start(StartCmd)
    }
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

    /// get_config_file_path returns the configuration file path based on the command-line arguments
    /// and the configuration folder.
    pub fn get_config_file_path(&self) -> Result<PathBuf> {
        Ok(self.get_config_dir()?.join(CONFIG_FILE))
    }

    /// get_genesis_file_path returns the genesis file path based on the command-line arguments and
    /// the configuration folder.
    pub fn get_genesis_file_path(&self) -> Result<PathBuf> {
        Ok(self.get_config_dir()?.join(GENESIS_FILE))
    }

    /// get_log_level_or_default returns the log level from the command-line or the default value.
    pub fn get_log_level_or_default(&self) -> LogLevel {
        self.log_level.unwrap_or_default()
    }

    /// get_priv_validator_key_file_path returns the private validator key file path based on the
    /// configuration folder.
    pub fn get_priv_validator_key_file_path(&self) -> Result<PathBuf> {
        Ok(self.get_config_dir()?.join(PRIV_VALIDATOR_KEY_FILE))
    }

    /// load_config returns a configuration compiled from the input parameters
    pub fn load_config(&self) -> Result<Config> {
        let config_file = self.get_config_file_path()?;

        let mut config: Config = config::Config::builder()
            .add_source(config::File::from(config_file))
            .add_source(config::Environment::with_prefix("MALACHITE").separator("__"))
            .build()?
            .try_deserialize()?;

        if let Some(log_level) = self.log_level {
            config.logging.log_level = log_level;
        }

        if let Some(log_format) = self.log_format {
            config.logging.log_format = log_format;
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

#[cfg(test)]
mod tests {
    use malachite_node::config::LogLevel;

    use super::*;

    #[test]
    fn args_struct() {
        let args = Args::parse_from([
            "test",
            "--log-level",
            "warn",
            "--log-format",
            "json",
            "init",
        ]);
        assert_eq!(args.log_level, Some(LogLevel::Warn));
        assert_eq!(args.log_format, Some(LogFormat::Json));
        assert!(matches!(args.command, Commands::Init(_)));

        let args = Args::parse_from(["test", "start"]);
        assert_eq!(args.log_level, None);
        assert!(matches!(args.command, Commands::Start(_)));
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
