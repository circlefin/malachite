//! Distributed testnet command

use std::path::Path;
use std::time::Duration;

use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use itertools::Itertools;
use tracing::info;

use malachitebft_app::Node;
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
    pub fn run<N>(&self, node: &N, home_dir: &Path, logging: LoggingConfig) -> Result<()>
    where
        N: Node,
    {
        let runtime = match self.runtime {
            RuntimeFlavour::SingleThreaded => RuntimeConfig::SingleThreaded,
            RuntimeFlavour::MultiThreaded(n) => RuntimeConfig::MultiThreaded { worker_threads: n },
        };

        distributed_testnet(
            node,
            self.nodes,
            home_dir,
            runtime,
            self.machines.clone(),
            self.enable_discovery,
            self.bootstrap_protocol,
            self.selector,
            self.num_outbound_peers,
            self.num_inbound_peers,
            self.ephemeral_connection_timeout_ms,
            self.bootstrap_set_size,
            self.transport,
            logging,
            self.deterministic,
        )
        .map_err(|e| {
            eyre!(
                "Failed to generate distributed testnet configuration: {:?}",
                e
            )
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn distributed_testnet<N>(
    node: &N,
    nodes: usize,
    home_dir: &Path,
    runtime: RuntimeConfig,
    machines: Vec<String>,
    enable_discovery: bool,
    bootstrap_protocol: BootstrapProtocol,
    selector: Selector,
    num_outbound_peers: usize,
    num_inbound_peers: usize,
    ephemeral_connection_timeout_ms: u64,
    bootstrap_set_size: usize,
    transport: TransportProtocol,
    logging: LoggingConfig,
    deterministic: bool,
) -> Result<()>
where
    N: Node,
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

        save_config(
            &args.get_config_file_path()?,
            &generate_distributed_config(
                i,
                nodes,
                runtime,
                machines.clone(),
                enable_discovery,
                bootstrap_protocol,
                selector,
                num_outbound_peers,
                num_inbound_peers,
                ephemeral_connection_timeout_ms,
                bootstrap_set_size,
                transport,
                logging,
            ),
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

const CONSENSUS_BASE_PORT: usize = 27000;
const MEMPOOL_BASE_PORT: usize = 28000;
const METRICS_BASE_PORT: usize = 29000;

/// Generate configuration for node "index" out of "total" number of nodes.
#[allow(clippy::too_many_arguments)]
fn generate_distributed_config(
    index: usize,
    _total: usize,
    runtime: RuntimeConfig,
    machines: Vec<String>,
    enable_discovery: bool,
    bootstrap_protocol: BootstrapProtocol,
    selector: Selector,
    num_outbound_peers: usize,
    num_inbound_peers: usize,
    ephemeral_connection_timeout_ms: u64,
    bootstrap_set_size: usize,
    transport: TransportProtocol,
    logging: LoggingConfig,
) -> Config {
    let machine = machines[index % machines.len()].clone();
    let consensus_port = CONSENSUS_BASE_PORT + (index / machines.len());
    let mempool_port = MEMPOOL_BASE_PORT + (index / machines.len());
    let metrics_port = METRICS_BASE_PORT + (index / machines.len());

    Config {
        moniker: format!("test-{}", index),
        consensus: ConsensusConfig {
            vote_sync: VoteSyncConfig {
                mode: VoteSyncMode::RequestResponse,
            },
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                protocol: PubSubProtocol::default(),
                listen_addr: transport.multiaddr(&machine, consensus_port),
                persistent_peers: if enable_discovery {
                    let peers =
                        ((index.saturating_sub(bootstrap_set_size))..index).collect::<Vec<_>>();

                    peers
                        .iter()
                        .unique()
                        .map(|j| {
                            transport.multiaddr(
                                &machines[j % machines.len()].clone(),
                                CONSENSUS_BASE_PORT + (j / machines.len()),
                            )
                        })
                        .collect()
                } else {
                    let peers = (0..index).collect::<Vec<_>>();

                    peers
                        .iter()
                        .map(|j| {
                            transport.multiaddr(
                                &machines[*j % machines.len()],
                                CONSENSUS_BASE_PORT + (*j / machines.len()),
                            )
                        })
                        .collect()
                },
                discovery: DiscoveryConfig {
                    enabled: enable_discovery,
                    bootstrap_protocol,
                    selector,
                    num_outbound_peers,
                    num_inbound_peers,
                    ephemeral_connection_timeout: Duration::from_millis(
                        ephemeral_connection_timeout_ms,
                    ),
                },
                transport,
                ..Default::default()
            },
        },
        mempool: MempoolConfig {
            p2p: P2pConfig {
                protocol: PubSubProtocol::default(),
                listen_addr: transport.multiaddr(&machine, mempool_port),
                persistent_peers: vec![],
                discovery: DiscoveryConfig {
                    enabled: false,
                    bootstrap_protocol,
                    selector,
                    num_outbound_peers: 0,
                    num_inbound_peers: 0,
                    ephemeral_connection_timeout: Duration::from_secs(0),
                },
                transport,
                ..Default::default()
            },
            max_tx_count: 10000,
            gossip_batch_size: 0,
        },
        value_sync: ValueSyncConfig {
            enabled: false,
            status_update_interval: Duration::from_secs(0),
            request_timeout: Duration::from_secs(0),
        },
        metrics: MetricsConfig {
            enabled: true,
            listen_addr: format!("{machine}:{metrics_port}").parse().unwrap(),
        },
        logging,
        runtime,
        test: TestConfig::default(),
    }
}
