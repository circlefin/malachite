use clap::Parser;
use tracing::info;

use crate::error::Error;
use malachite_app::Node;
use malachite_config::MetricsConfig;

use crate::metrics;

#[derive(Parser, Debug, Clone, Default, PartialEq)]
pub struct StartCmd {
    #[clap(long)]
    pub start_height: Option<u64>,
}

impl StartCmd {
    pub async fn run<N>(&self, node: &N, metrics: Option<MetricsConfig>) -> Result<(), Error>
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
pub async fn start<N>(node: &N, metrics: Option<MetricsConfig>) -> Result<(), Error>
where
    N: Node,
{
    // Enable Prometheus
    if let Some(metrics) = metrics {
        tokio::spawn(metrics::serve(metrics.clone()));
    }

    // Start the node
    node.run().await;

    // Todo: refactor Node trait Node::run to return error messages.
    Ok(())
}
