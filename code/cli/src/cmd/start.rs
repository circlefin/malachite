use color_eyre::eyre::Result;

use malachite_actors::util::make_node_actor;
use malachite_node::config::Config;
use malachite_test::{Address, PrivateKey, ValidatorSet};
use tracing::info;

pub async fn run(sk: PrivateKey, cfg: Config, vs: ValidatorSet) -> Result<()> {
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
