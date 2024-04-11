//! Node command-line interface configuration
//!
//! The node CLI reads configuration from the configuration file provided with the
//! `--config` parameter. You can override these values on the command-line.
//!
//! The configuration is stored in the `Args` structure.
//! `clap` parses the command-line parameters into this structure.
//! `confy` uses `serde` to read/write the configuration between the config file and the structure.
//!

use crate::logging::DebugSection;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use clap::{CommandFactory, Parser, Subcommand};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Developer's note: when adding a new parameter, update the merge method!
#[derive(Parser, Clone, Debug, Default, Serialize, Deserialize)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Config file path
    #[arg(short, long, value_name = "FILE")]
    #[serde(skip)]
    config: Option<PathBuf>,

    /// Base64-encoded private key
    #[clap(long, default_value="", hide_default_value=true, value_name = "BASE64_STRING", env="PRIVATE_KEY", value_parser = |s: &str| {BASE64_STANDARD.decode(s)})]
    #[serde(with = "serde_base64")]
    pub private_key: std::vec::Vec<u8>, // Keep the fully qualified path for Vec<u8> or else clap will not be able to parse it: https://github.com/clap-rs/clap/issues/4481.

    /// Index of this node in the validator set (0, 1, or 2)
    #[clap(short, long, value_name = "INDEX", env = "INDEX", required = true)]
    pub index: usize,

    /// Validator voting power
    #[clap(
        short = 'p',
        long = "power",
        default_value_t = 1,
        hide_default_value = true,
        value_name = "POWER",
        env = "VOTING_POWER"
    )]
    pub voting_power: u64,

    #[clap(
        short,
        long = "debug",
        help = "Enable debug output for the given comma-separated sections",
        value_enum,
        value_delimiter = ','
    )]
    #[serde(with = "serde_debug_list")]
    pub debug: Vec<DebugSection>,

    #[command(subcommand)]
    #[serde(skip)]
    pub command: Commands,
}

#[derive(Subcommand, Clone, Debug, Default, Serialize, Deserialize)]
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
        let mut cfg: Args;

        // Get command-line parameters
        let cli_cfg = Args::parse();

        // Load config
        if let Some(config_path) = cli_cfg.config.as_deref() {
            cfg = confy::load_path(config_path)
                .map_err(|e| format!("Error loading config file: {e}"))
                .unwrap();
        } else {
            // Get default config
            let app = Args::command();
            let app_name = app.get_name();
            cfg = confy::load(app_name, None).unwrap_or_default();
        }

        // Merge command-line parameters into loaded config
        cfg.merge(cli_cfg);

        // If a private key was not provided, generate a random set of bytes
        if cfg.private_key.is_empty() {
            let mut private_key = [0u8; 32];
            OsRng.fill_bytes(&mut private_key);
            cfg.private_key = private_key.into();
        }

        // Save config on "Init" command
        if let Commands::Init = cfg.command {
            confy::store_path(cfg.config.as_deref().unwrap(), &cfg).unwrap();
        }

        // Temporarily keep hard-coded voting powers for backwards compatibility
        const VOTING_POWERS: [u64; 3] = [5, 20, 10];
        if cfg.index < VOTING_POWERS.len() {
            cfg.voting_power = VOTING_POWERS[cfg.index];
        }

        tracing::debug!("{:?}", cfg);
        cfg
    }

    /// merge the configuration from another instance.
    fn merge(&mut self, other: Args) {
        // Config file is only useful on the command-line
        self.config = other.config;

        if !other.private_key.is_empty() {
            self.private_key = other.private_key;
        }

        if other.index != 0 {
            self.index = other.index;
        }

        if other.voting_power != 0 {
            self.voting_power = other.voting_power;
        }

        if !other.debug.is_empty() {
            self.debug = other.debug;
        }

        // Command is only useful on the command-line
        self.command = other.command;
    }
}

// Serde base64-encoded String serializer/deserializer for confy file operations.
mod serde_base64 {
    use base64::prelude::BASE64_STANDARD;
    use base64::Engine;
    use serde::{Deserialize, Serializer};

    pub fn serialize<S>(s: &Vec<u8>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ser.serialize_str(BASE64_STANDARD.encode(s).as_str())
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(de)?;
        BASE64_STANDARD
            .decode(s)
            .map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

// Serde comma-separated String serializer/deserializer for Vec<DebugSection>.
mod serde_debug_list {
    use crate::logging::DebugSection;
    use clap::ValueEnum;
    use serde::{Deserialize, Serializer};

    pub fn serialize<S>(s: &[DebugSection], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let r = s
            .iter()
            .map(|s| format!("{:?}", s).to_lowercase())
            .collect::<Vec<String>>()
            .join(",");
        ser.serialize_str(r.as_str())
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Vec<DebugSection>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(de)?;
        if s.is_empty() {
            return Ok(vec![]);
        }
        s.split(',')
            .map(|s| DebugSection::from_str(s, true).map_err(serde::de::Error::custom))
            .collect()
    }
}
