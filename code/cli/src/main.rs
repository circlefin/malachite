use std::time::Duration;

use malachite_actors::node::Msg;
use malachite_actors::util::make_node_actor;
use malachite_test::{PrivateKey, ValidatorSet};

use config::Genesis;
use tracing::{debug, info};

use crate::logging::LogLevel;

mod config;
mod logging;

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = config::Args::new();
    let cfg = config::Config::load(&args)?;

    logging::init(LogLevel::Debug, &cfg.debug);
    debug!("Configuration loaded: {:?}", cfg);

    if let config::Commands::Init = args.command {
        cfg.save();
        debug!("Configuration saved to {:?}.", cfg.config_file);
        if !cfg.genesis_file.exists() {
            Genesis::default().save(&cfg.genesis_file);
            debug!("Sample genesis saved to {:?}.", cfg.genesis_file);
        }
        return Ok(());
    }

    let genesis = Genesis::load(&cfg)?;

    // Todo: simplify this and make it more robust.
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&cfg.test.private_key[0..32]);
    let sk = PrivateKey::from(pk);
    let mut address = [0u8; 20];
    address.copy_from_slice(&sk.public_key().hash()[0..20]);
    let val_address = malachite_test::Address::new(address);
    let vs = ValidatorSet::new(
        genesis
            .validators
            .into_iter()
            .map(|v| v.into())
            .collect::<Vec<_>>(),
    );
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
