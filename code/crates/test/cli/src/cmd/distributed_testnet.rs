//! Distributed testnet command

use std::path::Path;
use std::time::Duration;

use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use tracing::info;

use malachitebft_app::node::{
    CanGeneratePrivateKey, CanMakeDistributedConfig, CanMakeGenesis, CanMakePrivateKeyFile,
    MakeConfigSettings, Node,
};
use malachitebft_config::*;

use crate::args::Args;
use crate::cmd::testnet::RuntimeFlavour;
use crate::file::{save_config, save_genesis, save_priv_validator_key};

#[derive(Parser, Debug, Clone, PartialEq)]
pub struct DistributedTestnetCmd {
    /// Number of validator nodes in the testnet
    #[clap(short, long)]
    pub nodes: usize,

    /// Generate deterministic private keys for reproducibility
    #[clap(short, long)]
    pub deterministic: bool,

    /// The flavor of Tokio runtime to use.
    /// Possible values:
    /// - "single-threaded": A single threaded runtime (default)
    /// - "multi-threaded:N":  A multi-threaded runtime with as N worker threads
    ///   Use a value of 0 for N to use the number of cores available on the system.
    #[clap(short, long, default_value = "single-threaded", verbatim_doc_comment)]
    pub runtime: RuntimeFlavour,

    /// The IPs of the available machines in the network (comma separated) on which to run the nodes
    #[clap(long, value_delimiter = ',', verbatim_doc_comment)]
    pub machines: Vec<String>,

    /// Enable peer discovery.
    /// If enabled, the node will attempt to discover other nodes in the network
    #[clap(long, default_value = "false")]
    pub enable_discovery: bool,

    /// Bootstrap protocol
    /// The protocol used to bootstrap the discovery mechanism
    /// Possible values:
    /// - "kademlia": Kademlia
    /// - "full": Full mesh (default)
    #[clap(long, default_value = "full", verbatim_doc_comment)]
    pub bootstrap_protocol: BootstrapProtocol,

    /// Selector
    /// The selection strategy used to select persistent peers
    /// Possible values:
    /// - "kademlia": Kademlia-based selection, only available with the Kademlia bootstrap protocol
    /// - "random": Random selection (default)
    #[clap(long, default_value = "random", verbatim_doc_comment)]
    pub selector: Selector,

    /// Number of outbound peers
    #[clap(long, default_value = "20", verbatim_doc_comment)]
    pub num_outbound_peers: usize,

    /// Number of inbound peers
    /// Must be greater than or equal to the number of outbound peers
    #[clap(long, default_value = "20", verbatim_doc_comment)]
    pub num_inbound_peers: usize,

    /// Ephemeral connection timeout
    /// The duration in milliseconds an ephemeral connection is kept alive
    #[clap(long, default_value = "5000", verbatim_doc_comment)]
    pub ephemeral_connection_timeout_ms: u64,

    /// The size of the bootstrap set.
    #[clap(long, default_value = "1", verbatim_doc_comment)]
    pub bootstrap_set_size: usize,

    /// The transport protocol to use for P2P communication
    /// Possible values:
    /// - "quic": QUIC (default)
    /// - "tcp": TCP + Noise
    #[clap(short, long, default_value = "quic", verbatim_doc_comment)]
    pub transport: TransportProtocol,
}

impl DistributedTestnetCmd {
    /// Execute the testnet command
    pub fn run<N>(&self, node: &N, home_dir: &Path) -> Result<()>
    where
        N: Node
            + CanGeneratePrivateKey
            + CanMakeDistributedConfig
            + CanMakeGenesis
            + CanMakePrivateKeyFile,
    {
        let runtime = match self.runtime {
            RuntimeFlavour::SingleThreaded => RuntimeConfig::SingleThreaded,
            RuntimeFlavour::MultiThreaded(n) => RuntimeConfig::MultiThreaded { worker_threads: n },
        };

        let settings = MakeConfigSettings {
            runtime,
            transport: self.transport,
            discovery: DiscoveryConfig {
                enabled: self.enable_discovery,
                bootstrap_protocol: self.bootstrap_protocol,
                selector: self.selector,
                num_outbound_peers: self.num_outbound_peers,
                num_inbound_peers: self.num_inbound_peers,
                max_connections_per_peer: self.num_outbound_peers + self.num_inbound_peers,
                ephemeral_connection_timeout: Duration::from_millis(
                    self.ephemeral_connection_timeout_ms,
                ),
            },
            value_sync: Default::default(),
        };

        distributed_testnet(
            node,
            self.nodes,
            home_dir,
            self.machines.clone(),
            self.bootstrap_set_size,
            self.deterministic,
            settings,
        )
        .map_err(|e| {
            eyre!(
                "Failed to generate distributed testnet configuration: {:?}",
                e
            )
        })
    }
}

fn distributed_testnet<N>(
    node: &N,
    nodes: usize,
    home_dir: &Path,
    machines: Vec<String>,
    bootstrap_set_size: usize,
    deterministic: bool,
    settings: MakeConfigSettings,
) -> Result<()>
where
    N: Node
        + CanGeneratePrivateKey
        + CanMakeDistributedConfig
        + CanMakeGenesis
        + CanMakePrivateKeyFile,
{
    let private_keys = crate::new::generate_private_keys(node, nodes, deterministic);
    let public_keys = private_keys
        .iter()
        .map(|pk| node.get_public_key(pk))
        .collect();
    let genesis = crate::new::generate_genesis(node, public_keys, deterministic);

    for (i, private_key) in private_keys.iter().enumerate().take(nodes) {
        let node_home_dir = home_dir
            .join((i % machines.len()).to_string())
            .join(i.to_string());

        info!(
            id = %i,
            home = %node_home_dir.display(),
            "Generating configuration for node..."
        );

        let args = Args {
            home: Some(node_home_dir),
            ..Args::default()
        };

        save_config::<N>(
            &args.get_config_file_path()?,
            &N::make_distributed_config(i, nodes, machines.clone(), bootstrap_set_size, settings),
        )?;

        let priv_validator_key = node.make_private_key_file((*private_key).clone());
        save_priv_validator_key(
            node,
            &args.get_priv_validator_key_file_path()?,
            &priv_validator_key,
        )?;

        save_genesis(node, &args.get_genesis_file_path()?, &genesis)?;
    }

    Ok(())
}
