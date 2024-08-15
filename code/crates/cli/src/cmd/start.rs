use clap::Parser;
use color_eyre::eyre::Result;
use tracing::{info, Instrument};

use malachite_node::config::{App, Config};
use malachite_test::utils::test::SpawnNodeActor;
use malachite_test::{Address, PrivateKey, ValidatorSet};

use malachite_starknet_app::spawn::SpawnStarknetNode;

use crate::metrics;

#[derive(Parser, Debug, Clone, Default, PartialEq)]
pub struct StartCmd;

impl StartCmd {
    pub async fn run(&self, sk: PrivateKey, cfg: Config, vs: ValidatorSet) -> Result<()> {
        let val_address = Address::from_public_key(&sk.public_key());
        let moniker = cfg.moniker.clone();

        let span = tracing::error_span!("node", %moniker);
        let _enter = span.enter();

        if cfg.metrics.enabled {
            tokio::spawn(metrics::serve(cfg.metrics.clone()).instrument(span.clone()));
        }

        info!("Node is starting...");

        let (actor, handle) = match cfg.app {
            App::Starknet => {
                SpawnStarknetNode::spawn_node_actor(cfg, vs, sk.clone(), sk, val_address, None)
                    .await
            }
        };

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

        handle.await?;

        info!("Node has stopped");

        Ok(())
    }
}
