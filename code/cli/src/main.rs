use malachite_actors::util::make_node_actor;
use malachite_node::config::Config;
use malachite_test::{Address, PrivateKey, ValidatorSet};
use rand::rngs::OsRng;

use args::Commands;
use example::{generate_config, generate_genesis, generate_private_key};
use tracing::{debug, info};

use crate::logging::LogLevel;

mod args;
mod example;
mod init;
mod logging;

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = args::Args::new();

    logging::init(LogLevel::Debug, &args.debug);
    debug!("Command-line parameters: {:?}", args);

    if let Commands::Init = args.command {
        init::run(
            args.get_config_file_path()?,
            args.get_genesis_file_path()?,
            args.get_priv_validator_key_file_path()?,
            args.index.unwrap_or(0),
        );
        return Ok(());
    }

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

    let val_address = Address::from_public_key(&sk.public_key());
    let moniker = cfg.moniker.clone();

    info!("[{}] Starting...", &cfg.moniker);

    let (tx_decision, mut rx_decision) = tokio::sync::mpsc::channel(32);
    let (actor, handle) = make_node_actor(vs, sk, val_address, tx_decision).await;

    tokio::spawn({
        let actor = actor.clone();
        async move {
            tokio::signal::ctrl_c().await.unwrap();
            info!("[{moniker}] Shutting down...");
            actor.stop(None);
        }
    });

    while let Some((height, round, value)) = rx_decision.recv().await {
        info!(
            "[{}] Decision at height {height} and round {round}: {value:?}",
            &cfg.moniker
        );
    }

    handle.await?;

    Ok(())
}
