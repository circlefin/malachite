use color_eyre::eyre::eyre;
use malachite_cli::args::{Args, Commands};
use malachite_cli::{logging, runtime};
use malachite_starknet_app::node::StarknetNode;
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
        malachite_config::load_config(&path, None)
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
        config: None,
        genesis_file: args.get_genesis_file_path().unwrap(),
        private_key_file: args.get_priv_validator_key_file_path().unwrap(),
    };

    match &args.command {
        Commands::Start(cmd) => {
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

            let node = &StarknetNode {
                config: Some(config),
                genesis_file: args.get_genesis_file_path().unwrap(),
                private_key_file: args.get_priv_validator_key_file_path().unwrap(),
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
    }
}
