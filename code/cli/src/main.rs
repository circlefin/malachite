use color_eyre::eyre::Result;
use rand::rngs::OsRng;
use tracing::debug;

use malachite_node::config::Config;
use malachite_test::{PrivateKey, ValidatorSet};

use crate::args::{Args, Commands};
use crate::example::{generate_config, generate_genesis, generate_private_key};
use crate::logging::LogLevel;

mod args;
mod cmd;
mod example;
mod logging;

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<()> {
    let args = Args::new();

    logging::init(LogLevel::Debug, &args.debug);

    debug!("Command-line parameters: {args:?}");

    match args.command {
        Commands::Init => init(&args),
        Commands::Start => start(&args).await,
    }
}

fn init(args: &Args) -> Result<()> {
    cmd::init::run(
        &args.get_config_file_path()?,
        &args.get_genesis_file_path()?,
        &args.get_priv_validator_key_file_path()?,
        args.index.unwrap_or(0),
    )
}

async fn start(args: &Args) -> Result<()> {
    let cfg: Config = match args.index {
        None => args.load_config()?,
        Some(index) => generate_config(index),
    };

    let sk: PrivateKey = match args.index {
        None => args
            .load_private_key()
            .unwrap_or_else(|_| PrivateKey::generate(OsRng)),
        Some(index) => generate_private_key(index),
    };

    let vs: ValidatorSet = match args.index {
        None => args.load_genesis()?,
        Some(_) => generate_genesis(),
    };

    cmd::start::run(sk, cfg, vs).await
}
