use color_eyre::eyre::eyre;
use malachitebft_starknet_host::node::StarknetNode;
use malachitebft_test_cli::args::{Args, Commands};
use malachitebft_test_cli::{logging, runtime};
use tracing::{error, info, trace};

// Use jemalloc on Linux
#[cfg(target_os = "linux")]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
pub fn main() -> color_eyre::Result<()> {
    color_eyre::install().expect("Failed to install global error handler");

    // Load command-line arguments and possible configuration file.
    let args = Args::new();
    let opt_config_file_path = args
        .get_config_file_path()
        .map_err(|error| eyre!("Failed to get configuration file path: {:?}", error));
    let opt_config = opt_config_file_path.and_then(|path| {
        malachitebft_config::load_config(&path, None)
            .map_err(|error| eyre!("Failed to load configuration file: {:?}", error))
    });

    // Override logging configuration (if exists) with optional command-line parameters.
    let mut logging = opt_config.as_ref().map(|c| c.logging).unwrap_or_default();
    if let Some(log_level) = args.log_level {
        logging.log_level = log_level;
    }
    if let Some(log_format) = args.log_format {
        logging.log_format = log_format;
    }

    // This is a drop guard responsible for flushing any remaining logs when the program terminates.
    // It must be assigned to a binding that is not _, as _ will result in the guard being dropped immediately.
    let _guard = logging::init(logging.log_level, logging.log_format);

    trace!("Command-line parameters: {args:?}");

    let node = &StarknetNode {
        home_dir: args.get_home_dir().unwrap(),
        config: Default::default(), // placeholder, because `init` and `testnet` has no valid configuration file.
        genesis_file: args.get_genesis_file_path().unwrap(),
        private_key_file: args.get_priv_validator_key_file_path().unwrap(),
        start_height: Default::default(), // placeholder, because start_height is only valid in StartCmd.
    };

    match &args.command {
        Commands::Start(cmd) => {
            // Build configuration from valid configuration file and command-line parameters.
            let mut config = opt_config
                .map_err(|error| error!(%error, "Failed to load configuration."))
                .unwrap();
            config.logging = logging;
            let runtime = config.runtime;
            let metrics = if config.metrics.enabled {
                Some(config.metrics.clone())
            } else {
                None
            };

            info!(
                file = %args.get_config_file_path().unwrap_or_default().display(),
                "Loaded configuration",
            );
            trace!(?config, "Configuration");

            // Redefine the node with the valid configuration.
            let node = StarknetNode {
                home_dir: args.get_home_dir().unwrap(),
                config,
                genesis_file: args.get_genesis_file_path().unwrap(),
                private_key_file: args.get_priv_validator_key_file_path().unwrap(),
                start_height: cmd.start_height,
            };

            let rt = runtime::build_runtime(runtime)?;
            rt.block_on(cmd.run(node, metrics))
                .map_err(|error| eyre!("Failed to run start command {:?}", error))
        }
        Commands::Init(cmd) => cmd
            .run(
                node,
                &args.get_config_file_path().unwrap(),
                &args.get_genesis_file_path().unwrap(),
                &args.get_priv_validator_key_file_path().unwrap(),
                logging,
            )
            .map_err(|error| eyre!("Failed to run init command {:?}", error)),
        Commands::Testnet(cmd) => cmd
            .run(node, &args.get_home_dir().unwrap(), logging)
            .map_err(|error| eyre!("Failed to run testnet command {:?}", error)),
        Commands::DistributedTestnet(cmd) => cmd
            .run(node, &args.get_home_dir().unwrap(), logging)
            .map_err(|error| eyre!("Failed to run distributed testnet command {:?}", error)),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use clap::Parser;
    use color_eyre::eyre;
    use color_eyre::eyre::eyre;
    use malachitebft_config::LoggingConfig;
    use malachitebft_starknet_host::node::StarknetNode;
    use malachitebft_test_cli::args::{Args, Commands};
    use malachitebft_test_cli::cmd::init::*;

    #[test]
    fn running_init_creates_config_files() -> eyre::Result<()> {
        let tmp = tempfile::tempdir()?;
        let config_dir = tmp.path().join("config");

        let args = Args::parse_from(["test", "--home", tmp.path().to_str().unwrap(), "init"]);
        let cmd = InitCmd::default();

        let node = &StarknetNode {
            home_dir: tmp.path().to_owned(),
            config: Default::default(),
            genesis_file: PathBuf::from("genesis.json"),
            private_key_file: PathBuf::from("priv_validator_key.json"),
            start_height: Default::default(),
        };
        cmd.run(
            node,
            &args.get_config_file_path().unwrap(),
            &args.get_genesis_file_path().unwrap(),
            &args.get_priv_validator_key_file_path().unwrap(),
            LoggingConfig {
                log_level: args.log_level.unwrap_or_default(),
                log_format: args.log_format.unwrap_or_default(),
            },
        )
        .expect("Failed to run init command");

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

        let Commands::Testnet(ref cmd) = args.command else {
            return Err(eyre!("not testnet command"));
        };

        let node = &StarknetNode {
            home_dir: tmp.path().to_owned(),
            config: Default::default(),
            genesis_file: PathBuf::from("genesis.json"),
            private_key_file: PathBuf::from("priv_validator_key.json"),
            start_height: Default::default(),
        };
        cmd.run(
            node,
            &args.get_home_dir().unwrap(),
            LoggingConfig {
                log_level: args.log_level.unwrap_or_default(),
                log_format: args.log_format.unwrap_or_default(),
            },
        )
        .expect("Failed to run init command");

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
