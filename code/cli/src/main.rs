use std::time::Duration;

use malachite_actors::node::Msg;
use malachite_actors::util::make_node_actor;
use malachite_node::config::Config;
use malachite_test::{PrivateKey, ValidatorSet};
use rand::rngs::OsRng;

use args::Commands;
use args::{generate_test_genesis, generate_test_private_key};
use args::{save_config, save_genesis, save_priv_validator_key};
use tracing::{debug, info};

use crate::logging::LogLevel;

mod args;
mod logging;

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = args::Args::new();

    logging::init(LogLevel::Debug, &args.debug);
    debug!("Command-line parameters: {:?}", args);

    if let Commands::Init = args.command {
        let config_file = args.get_config_file_path()?;
        let genesis_file = args.get_genesis_file_path()?;
        let priv_validator_key_file = args.get_priv_validator_key_file_path()?;
        // Save default configuration
        if !config_file.exists() {
            debug!("Saving configuration to {:?}.", config_file);
            save_config(&config_file, &Config::default());
        }
        // Save default genesis
        if !genesis_file.exists() {
            debug!("Saving test genesis to {:?}.", genesis_file);
            save_genesis(&genesis_file, &generate_test_genesis());
        }
        // Save default priv_validator_key
        if !priv_validator_key_file.exists() {
            debug!("Saving private key to {:?}.", priv_validator_key_file);
            let index = args.index.unwrap_or(0);
            save_priv_validator_key(&priv_validator_key_file, &generate_test_private_key(index));
        }
        return Ok(());
    }

    let cfg: Config = args.clone().try_into()?;
    let sk: PrivateKey = match args.index {
        None => args
            .clone()
            .try_into()
            .unwrap_or_else(|_| PrivateKey::generate(OsRng)),
        Some(index) => generate_test_private_key(index),
    };
    let vs: ValidatorSet = match args.index {
        None => args.clone().try_into()?,
        Some(_) => generate_test_genesis(),
    };

    let mut address = [0u8; 20];
    address.copy_from_slice(&sk.public_key().hash()[0..20]);
    let val_address = malachite_test::Address::new(address);
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

    tokio::time::sleep(Duration::from_secs(1)).await;

    actor.cast(Msg::Start)?;

    while let Some((height, round, value)) = rx_decision.recv().await {
        info!(
            "[{}] Decision at height {height} and round {round}: {value:?}",
            &cfg.moniker
        );
    }

    handle.await?;

    Ok(())
}
