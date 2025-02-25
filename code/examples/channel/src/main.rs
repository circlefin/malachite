//! Example application using channels

use config::Config;
use eyre::{eyre, Result};
use tracing::info;

use malachitebft_app_channel::app::node::Node;
use malachitebft_test::Height;
use malachitebft_test_cli::args::{Args, Commands};
use malachitebft_test_cli::cmd::init::InitCmd;
use malachitebft_test_cli::cmd::start::StartCmd;
use malachitebft_test_cli::cmd::testnet::TestnetCmd;
use malachitebft_test_cli::{logging, runtime};

mod app;
mod config;
mod metrics;
mod node;
mod state;
mod store;
mod streaming;

use node::App;

/// Main entry point for the application
///
/// This function:
/// - Parses command-line arguments
/// - Loads configuration from file
/// - Initializes logging system
/// - Sets up error handling
/// - Creates and runs the application node
fn main() -> Result<()> {
    color_eyre::install()?;

    // Load command-line arguments and possible configuration file.
    let args = Args::new();

    // Parse the input command.
    match &args.command {
        Commands::Start(cmd) => start(&args, cmd),
        Commands::Init(cmd) => init(&args, cmd),
        Commands::Testnet(cmd) => testnet(&args, cmd),
        _ => unimplemented!(),
    }
}

fn start(args: &Args, cmd: &StartCmd) -> Result<()> {
    // Setup the application
    let app = App {
        home_dir: args.get_home_dir()?,
        config_file: args.get_config_file_path()?,
        genesis_file: args.get_genesis_file_path()?,
        private_key_file: args.get_priv_validator_key_file_path()?,
        start_height: cmd.start_height.map(Height::new),
    };

    let config: Config = app.load_config()?;

    // This is a drop guard responsible for flushing any remaining logs when the program terminates.
    // It must be assigned to a binding that is not _, as _ will result in the guard being dropped immediately.
    let _guard = logging::init(config.logging.log_level, config.logging.log_format);

    let rt = runtime::build_runtime(config.runtime)?;

    info!(moniker = %config.moniker, "Starting Malachite");

    // Start the node
    rt.block_on(app.run())
        .map_err(|error| eyre!("Failed to run the application node: {error}"))
}

fn init(args: &Args, cmd: &InitCmd) -> Result<()> {
    // Setup the application
    let app = App {
        home_dir: args.get_home_dir()?,
        config_file: args.get_config_file_path()?,
        genesis_file: args.get_genesis_file_path()?,
        private_key_file: args.get_priv_validator_key_file_path()?,
        start_height: None,
    };

    cmd.run(
        &app,
        &args.get_config_file_path()?,
        &args.get_genesis_file_path()?,
        &args.get_priv_validator_key_file_path()?,
    )
    .map_err(|error| eyre!("Failed to run init command {error:?}"))
}

fn testnet(args: &Args, cmd: &TestnetCmd) -> Result<()> {
    // Setup the application
    let app = App {
        home_dir: args.get_home_dir()?,
        config_file: args.get_config_file_path()?,
        genesis_file: args.get_genesis_file_path()?,
        private_key_file: args.get_priv_validator_key_file_path()?,
        start_height: Some(Height::new(1)), // We always start at height 1
    };

    cmd.run(&app, &args.get_home_dir()?)
        .map_err(|error| eyre!("Failed to run testnet command {:?}", error))
}
