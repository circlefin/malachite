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

#[cfg(test)]
mod tests {
    use clap::Parser;
    use color_eyre::eyre;
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn running_init_creates_config_files() -> eyre::Result<()> {
        let tmp = tempfile::tempdir()?;

        let config = tmp.path().join("config.toml");
        let genesis = tmp.path().join("genesis.json");

        let args = Args::parse_from([
            "test",
            "--config",
            &config.display().to_string(),
            "--genesis",
            &genesis.display().to_string(),
            "init",
        ]);

        init(&args)?;

        let files = fs::read_dir(tmp.path())?.flatten().collect::<Vec<_>>();

        assert!(has_file(&files, &config));
        assert!(has_file(&files, &genesis));

        Ok(())
    }

    fn has_file(files: &[fs::DirEntry], path: &PathBuf) -> bool {
        files.iter().any(|f| &f.path() == path)
    }
}
