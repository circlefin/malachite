use color_eyre::eyre::Result;

use malachite_actors::util::spawn_node_actor;
use malachite_node::config::Config;
use malachite_test::{Address, PrivateKey, ValidatorSet};
use tracing::{info, Instrument};

use crate::metrics::serve_metrics;

pub async fn run(sk: PrivateKey, cfg: Config, vs: ValidatorSet) -> Result<()> {
    let val_address = Address::from_public_key(&sk.public_key());
    let moniker = cfg.moniker.clone();

    let span = tracing::error_span!("node", moniker = %moniker);
    let _enter = span.enter();

    tokio::spawn(serve_metrics(cfg.metrics.clone()).instrument(span.clone()));

    info!("Node is starting...");

    let (tx_decision, mut rx_decision) = tokio::sync::mpsc::channel(32);
    let (actor, handle) = spawn_node_actor(cfg, vs, sk.clone(), sk, val_address, tx_decision).await;

    tokio::spawn({
        let actor = actor.clone();
        {
            async move {
                tokio::signal::ctrl_c().await.unwrap();
                info!("Shutting down...");
                actor.stop(None);
            }
        }
        .instrument(span.clone())
    });

    while let Some((height, round, value)) = rx_decision.recv().await {
        info!(
            "Decision at height {height} and round {round}: {:?}",
            value.id()
        );
    }

    handle.await?;

    Ok(())
}
