use std::path::PathBuf;

use clap::Parser;
use color_eyre::eyre::Result;
use tracing::{info, Instrument};

use malachite_node::config::{App, Config};
use malachite_node::Node;
use malachite_starknet_app::node::StarknetNode;

use crate::metrics;

#[derive(Parser, Debug, Clone, Default, PartialEq)]
pub struct StartCmd;

impl StartCmd {
    pub async fn run(
        &self,
        cfg: Config,
        private_key_file: PathBuf,
        genesis_file: PathBuf,
    ) -> Result<()> {
        let moniker = cfg.moniker.clone();

        let span = tracing::error_span!("node", %moniker);
        let _enter = span.enter();

        if cfg.metrics.enabled {
            tokio::spawn(metrics::serve(cfg.metrics.clone()).instrument(span.clone()));
        }

        info!("Node is starting...");

        let node = match cfg.app {
            App::Starknet => StarknetNode,
        };

        let priv_key_file = node.load_private_key_file(private_key_file)?;
        let private_key = node.load_private_key(priv_key_file);
        let genesis = node.load_genesis(genesis_file)?;

        let (actor, handle) = match cfg.app {
            App::Starknet => {
                use malachite_starknet_app::spawn::spawn_node_actor;
                spawn_node_actor(cfg, genesis, private_key, None).await
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
