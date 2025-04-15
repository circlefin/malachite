//! Command-line interface arguments for a basic implementation.
//!
//! Read configuration from the configuration files found in the directory
//! provided with the `--home` global parameter.
//!
//! The command-line parameters are stored in the `Args` structure.
//! `clap` parses the command-line parameters into this structure.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use directories::BaseDirs;

use crate::cmd::distributed_testnet::DistributedTestnetCmd;
use crate::cmd::dump_wal::DumpWalCmd;
use crate::cmd::init::InitCmd;
use crate::cmd::start::StartCmd;
use crate::cmd::testnet::TestnetCmd;
use crate::error::Error;

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

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Start node
    Start(StartCmd),

    /// Initialize configuration
    Init(InitCmd),

    /// Generate testnet configuration
    Testnet(TestnetCmd),

    /// Generate distributed testnet configuration
    DistributedTestnet(DistributedTestnetCmd),

    /// Dump WAL entries
    DumpWal(DumpWalCmd),
}

impl Default for Commands {
    fn default() -> Self {
        Commands::Start(StartCmd::default())
    }
}

impl Args {
    /// new returns a new instance of the arguments.
    pub fn new() -> Args {
        Args::parse()
    }

    /// get_home_dir returns the application home folder.
    /// Typically, `$HOME/.malachite`, dependent on the operating system.
    pub fn get_home_dir(&self) -> Result<PathBuf, Error> {
        match self.home {
            Some(ref path) => Ok(path.clone()),
            None => Ok(BaseDirs::new()
                .ok_or(Error::DirPath)?
                .home_dir()
                .join(APP_FOLDER)),
        }
    }

    /// get_config_dir returns the configuration folder based on the home folder.
    pub fn get_config_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.get_home_dir()?.join("config"))
    }

    /// get_config_file_path returns the configuration file path based on the command-line arguments
    /// and the configuration folder.
    pub fn get_config_file_path(&self) -> Result<PathBuf, Error> {
        Ok(self.get_config_dir()?.join(CONFIG_FILE))
    }

    /// get_genesis_file_path returns the genesis file path based on the command-line arguments and
    /// the configuration folder.
    pub fn get_genesis_file_path(&self) -> Result<PathBuf, Error> {
        Ok(self.get_config_dir()?.join(GENESIS_FILE))
    }

    /// get_priv_validator_key_file_path returns the private validator key file path based on the
    /// configuration folder.
    pub fn get_priv_validator_key_file_path(&self) -> Result<PathBuf, Error> {
        Ok(self.get_config_dir()?.join(PRIV_VALIDATOR_KEY_FILE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args() {
        let args = Args::parse_from(["test", "init"]);
        assert!(matches!(args.command, Commands::Init(_)));

        let args = Args::parse_from(["test", "start"]);
        assert!(matches!(args.command, Commands::Start(_)));
    }

    #[test]
    fn parse_home_path() {
        let args = Args::parse_from(["test", "start", "--home", "/tmp"]);
        assert_eq!(
            args.get_config_file_path().unwrap(),
            PathBuf::from("/tmp/config/config.toml")
        );
        assert_eq!(
            args.get_genesis_file_path().unwrap(),
            PathBuf::from("/tmp/config/genesis.json")
        );
    }
}
