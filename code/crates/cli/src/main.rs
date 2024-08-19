use color_eyre::eyre::Result;
use tracing::{error, info, trace};

use malachite_node::config::{Config, RuntimeConfig};

use crate::args::{Args, Commands};
use crate::cmd::init::InitCmd;
use crate::cmd::keys::KeysCmd;
use crate::cmd::start::StartCmd;
use crate::cmd::testnet::TestnetCmd;

mod args;
mod cmd;
mod logging;
mod metrics;

pub fn main() -> Result<()> {
    color_eyre::install().expect("Failed to install global error handler");

    let args = Args::new();
    let config = args.load_config();
    let logging = config.as_ref().map(|c| c.logging).unwrap_or_default();

    logging::init(logging.log_level, logging.log_format);

    trace!("Command-line parameters: {args:?}");

    match &args.command {
        Commands::Start(cmd) => {
            let config = config.map_err(|e| {
                error!("Failed to load configuration: {e}");
                e
            })?;

            info!(
                "Loaded configuration from {:?}",
                args.get_config_file_path().unwrap_or_default().display()
            );

            start(&args, config, cmd)
        }
        Commands::Init(cmd) => init(&args, cmd),
        Commands::Keys(cmd) => keys(&args, cmd),
        Commands::Testnet(cmd) => testnet(&args, cmd),
    }
}

fn start(args: &Args, cfg: Config, cmd: &StartCmd) -> Result<()> {
    use tokio::runtime::Builder as RtBuilder;

    let mut builder = match cfg.runtime {
        RuntimeConfig::SingleThreaded => RtBuilder::new_current_thread(),
        RuntimeConfig::MultiThreaded { worker_threads } => {
            let mut builder = RtBuilder::new_multi_thread();
            if worker_threads > 0 {
                builder.worker_threads(worker_threads);
            }
            builder
        }
    };

    let rt = builder.enable_all().build()?;
    rt.block_on(cmd.run(
        cfg,
        args.get_priv_validator_key_file_path()?,
        args.get_genesis_file_path()?,
    ))
}

fn init(args: &Args, cmd: &InitCmd) -> Result<()> {
    cmd.run(
        &args.get_config_file_path()?,
        &args.get_genesis_file_path()?,
        &args.get_priv_validator_key_file_path()?,
        args.get_log_level_or_default(),
        args.log_format.unwrap_or_default(),
    )
}

fn keys(args: &Args, cmd: &KeysCmd) -> Result<()> {
    cmd.run(args)
}

fn testnet(args: &Args, cmd: &TestnetCmd) -> Result<()> {
    cmd.run(
        &args.get_home_dir()?,
        args.get_log_level_or_default(),
        args.log_format.unwrap_or_default(),
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
        let cmd = InitCmd::default();

        init(&args, &cmd)?;

        let files = fs::read_dir(&config_dir)?.flatten().collect::<Vec<_>>();

        assert!(has_file(&files, &config_dir.join("config.toml")));
        assert!(has_file(&files, &config_dir.join("genesis.json")));
        assert!(has_file(
            &files,
            &config_dir.join("priv_validator_key.json")
        ));

        Ok(())
    }

    #[test]
    fn running_testnet_creates_all_configs() -> eyre::Result<()> {
        let tmp = tempfile::tempdir()?;

        let args = Args::parse_from([
            "test",
            "--home",
            tmp.path().to_str().unwrap(),
            "testnet",
            "--nodes",
            "3",
        ]);

        let Commands::Testnet(ref testnet_args) = args.command else {
            panic!("not testnet command");
        };

        testnet(&args, testnet_args)?;

        let files = fs::read_dir(&tmp)?.flatten().collect::<Vec<_>>();

        assert_eq!(files.len(), 3);

        assert!(has_file(&files, &tmp.path().join("0")));
        assert!(has_file(&files, &tmp.path().join("1")));
        assert!(has_file(&files, &tmp.path().join("2")));

        for node in 0..3 {
            let node_dir = tmp.path().join(node.to_string()).join("config");
            let files = fs::read_dir(&node_dir)?.flatten().collect::<Vec<_>>();

            assert!(has_file(&files, &node_dir.join("config.toml")));
            assert!(has_file(&files, &node_dir.join("genesis.json")));
            assert!(has_file(&files, &node_dir.join("priv_validator_key.json")));
        }

        Ok(())
    }

    fn has_file(files: &[fs::DirEntry], path: &PathBuf) -> bool {
        files.iter().any(|f| &f.path() == path)
    }
}
