//! Node command-line interface configuration
//!
//! The node CLI reads configuration from the configuration file provided with the
//! `--config` parameter. Some configuration parameters can be overridden on the command-line.
//!
//! The command-line parameters are stored in the `Args` structure.
//! `clap` parses the command-line parameters into this structure.
//!

use crate::config::commands::Commands;
use crate::logging::DebugSection;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use clap::Parser;
use std::path::PathBuf;

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
    #[clap(short, long, default_value = "0", value_name = "INDEX", env = "INDEX")]
    pub index: usize,

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
}
