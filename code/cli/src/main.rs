use color_eyre::eyre::Result;
use tracing::debug;

use malachite_node::config::Config;
use malachite_test::{PrivateKey, ValidatorSet};

use crate::args::{Args, Commands, TestnetArgs};
use crate::logging::LogLevel;

mod args;
mod cmd;
mod logging;
mod priv_key;

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<()> {
    let args = Args::new();

    logging::init(LogLevel::Debug, &args.debug);

    debug!("Command-line parameters: {args:?}");

    match args.command {
        Commands::Init => init(&args),
        Commands::Start => start(&args).await,
        Commands::Testnet(ref testnet_args) => testnet(&args, testnet_args).await,
    }
}

fn init(args: &Args) -> Result<()> {
    cmd::init::run(
        &args.get_config_file_path()?,
        &args.get_genesis_file_path()?,
        &args.get_priv_validator_key_file_path()?,
    )
}

async fn start(args: &Args) -> Result<()> {
    let cfg: Config = args.load_config()?;
    let sk: PrivateKey = args.load_private_key()?;
    let vs: ValidatorSet = args.load_genesis()?;

    cmd::start::run(sk, cfg, vs).await
}

async fn testnet(args: &Args, testnet_args: &TestnetArgs) -> Result<()> {
    cmd::testnet::run(
        &args.get_home_dir()?,
        testnet_args.nodes,
        testnet_args.deterministic,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use clap::Parser;
    use color_eyre::eyre;

    use super::*;

    #[test]
    fn running_init_creates_config_files() -> eyre::Result<()> {
        let tmp = tempfile::tempdir()?;
        let config_dir = tmp.path().join("config");

        let args = Args::parse_from(["test", "--home", tmp.path().to_str().unwrap(), "init"]);

        init(&args)?;

        let files = fs::read_dir(&config_dir)?.flatten().collect::<Vec<_>>();

        dbg!(&files);

        assert!(has_file(&files, &config_dir.join("config.toml")));
        assert!(has_file(&files, &config_dir.join("genesis.json")));
        assert!(has_file(
            &files,
            &config_dir.join("priv_validator_key.json")
        ));

        Ok(())
    }

    fn has_file(files: &[fs::DirEntry], path: &PathBuf) -> bool {
        files.iter().any(|f| &f.path() == path)
    }
}
