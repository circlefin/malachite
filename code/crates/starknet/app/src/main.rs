use color_eyre::eyre::Context;

use malachitebft_app::node::Node;
use malachitebft_config::{LogFormat, LogLevel};
use malachitebft_starknet_host::codec::ProtobufCodec;
use malachitebft_starknet_host::node::{ConfigSource, StarknetNode};
use malachitebft_test_cli::args::{Args, Commands};
use malachitebft_test_cli::{logging, runtime};

// Use jemalloc on Linux
#[cfg(target_os = "linux")]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

pub fn main() -> color_eyre::Result<()> {
    color_eyre::install().expect("Failed to install global error handler");

    // Load command-line arguments and possible configuration file.
    let args = Args::new();

    let home_dir = args.get_home_dir()?;
    let config_file = args.get_config_file_path()?;

    match &args.command {
        Commands::Start(cmd) => {
            // Redefine the node with the valid configuration.
            let node =
                StarknetNode::new(home_dir, ConfigSource::File(config_file), cmd.start_height);

            let config = node.load_config()?;

            // This is a drop guard responsible for flushing any remaining logs when the program terminates.
            // It must be assigned to a binding that is not _, as _ will result in the guard being dropped immediately.
            let _guard = logging::init(config.logging.log_level, config.logging.log_format);

            let metrics = config.metrics.enabled.then(|| config.metrics.clone());

            let rt = runtime::build_runtime(config.runtime)?;

            rt.block_on(cmd.run(node, metrics))
                .wrap_err("Failed to run `start` command")
        }

        Commands::Init(cmd) => {
            let _guard = logging::init(LogLevel::Info, LogFormat::Plaintext);

            let node = &StarknetNode::new(home_dir.clone(), ConfigSource::Default, None);

            cmd.run(
                node,
                &config_file,
                &args.get_genesis_file_path().unwrap(),
                &args.get_priv_validator_key_file_path().unwrap(),
            )
            .wrap_err("Failed to run `init` command")
        }

        Commands::Testnet(cmd) => {
            let _guard = logging::init(LogLevel::Info, LogFormat::Plaintext);

            let node = &StarknetNode {
                home_dir: home_dir.clone(),
                config_source: ConfigSource::Default,
                start_height: None,
            };

            cmd.run(node, &home_dir)
                .wrap_err("Failed to run `testnet` command")
        }

        Commands::DistributedTestnet(cmd) => {
            let _guard = logging::init(LogLevel::Info, LogFormat::Plaintext);

            let node = &StarknetNode {
                home_dir: home_dir.clone(),
                config_source: ConfigSource::Default,
                start_height: None,
            };

            cmd.run(node, &home_dir)
                .wrap_err("Failed to run `distributed-testnet` command")
        }

        Commands::DumpWal(cmd) => {
            let _guard = logging::init(LogLevel::Info, LogFormat::Plaintext);

            cmd.run(ProtobufCodec)
                .wrap_err("Failed to run `dump-wal` command")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use clap::Parser;
    use color_eyre::eyre;
    use color_eyre::eyre::eyre;

    use malachitebft_starknet_host::node::{ConfigSource, StarknetNode};
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
            config_source: ConfigSource::Default,
            start_height: None,
        };

        cmd.run(
            node,
            &args.get_config_file_path().unwrap(),
            &args.get_genesis_file_path().unwrap(),
            &args.get_priv_validator_key_file_path().unwrap(),
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
            config_source: ConfigSource::Default,
            start_height: None,
        };

        cmd.run(node, &args.get_home_dir().unwrap())
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
