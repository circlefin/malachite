use clap::Parser;
use color_eyre::eyre;
use tracing::info;

use malachitebft_config::MetricsConfig;
use malachitebft_test::node::Node;

use crate::metrics;

#[derive(Parser, Debug, Clone, Default, PartialEq)]
pub struct StartCmd {
    #[clap(long)]
    pub start_height: Option<u64>,

    /// Only allow connections to/from persistent peers
    #[clap(long)]
    pub persistent_peers_only: bool,

    /// Run as a validator node.
    ///
    /// When set, the node loads its consensus private key, signs a validator proof
    /// binding the consensus key to the P2P peer ID, and advertises itself as a validator.
    /// This affects peer scoring and mesh prioritization in the gossip network.
    ///
    /// Without this flag the node does not advertise a validator identity or send
    /// a validator proof.
    #[clap(long)]
    pub validator: bool,
}

impl StartCmd {
    pub async fn run(&self, node: impl Node, metrics: Option<MetricsConfig>) -> eyre::Result<()> {
        info!("Node is starting...");

        start(node, metrics).await?;

        info!("Node has stopped");

        Ok(())
    }
}

/// start command to run a node.
pub async fn start(node: impl Node, metrics: Option<MetricsConfig>) -> eyre::Result<()> {
    // Enable Prometheus
    if let Some(metrics) = metrics {
        if metrics.enabled {
            tokio::spawn(metrics::serve(metrics.listen_addr));
        }
    }

    // Start the node
    node.run().await?;

    Ok(())
}
