use clap::Parser;
use color_eyre::eyre;
use tracing::info;

use malachite_app::Node;
use malachite_config::MetricsConfig;

use crate::metrics;

#[derive(Parser, Debug, Clone, Default, PartialEq)]
pub struct StartCmd {
    #[clap(long)]
    pub start_height: Option<u64>,
}

impl StartCmd {
    pub async fn run<N>(&self, node: &N, metrics: Option<MetricsConfig>) -> eyre::Result<()>
    where
        N: Node,
    {
        info!("Node is starting...");

        start(node, metrics).await?;

        info!("Node has stopped");

        Ok(())
    }
}

/// start command to run a node.
pub async fn start<N>(node: &N, metrics: Option<MetricsConfig>) -> eyre::Result<()>
where
    N: Node,
{
    // Enable Prometheus
    if let Some(metrics) = metrics {
        tokio::spawn(metrics::serve(metrics.clone()));
    }

    // Start the node
    node.run().await?;

    Ok(())
}
