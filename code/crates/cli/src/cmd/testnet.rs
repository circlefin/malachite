//! Testnet command

use std::path::Path;
use std::str::FromStr;

use bytesize::ByteSize;
use clap::Parser;
use color_eyre::eyre::Result;
use itertools::Itertools;
use rand::prelude::StdRng;
use rand::rngs::OsRng;
use rand::{seq::IteratorRandom, Rng, SeedableRng};
use tracing::info;

use malachite_common::{PrivateKey, PublicKey};
use malachite_node::config::*;
use malachite_node::Node;
use malachite_starknet_app::node::StarknetNode;

use crate::args::Args;
use crate::cmd::init::{save_config, save_genesis, save_priv_validator_key};

const MIN_VOTING_POWER: u64 = 8;
const MAX_VOTING_POWER: u64 = 15;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RuntimeFlavour {
    SingleThreaded,
    MultiThreaded(usize),
}

impl FromStr for RuntimeFlavour {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains(':') {
            match s.split_once(':') {
                Some(("multi-threaded", n)) => Ok(RuntimeFlavour::MultiThreaded(
                    n.parse()
                        .map_err(|_| "Invalid number of threads".to_string())?,
                )),
                _ => Err(format!("Invalid runtime flavour: {s}")),
            }
        } else {
            match s {
                "single-threaded" => Ok(RuntimeFlavour::SingleThreaded),
                "multi-threaded" => Ok(RuntimeFlavour::MultiThreaded(0)),
                _ => Err(format!("Invalid runtime flavour: {s}")),
            }
        }
    }
}

#[derive(Parser, Debug, Clone, PartialEq)]
pub struct TestnetCmd {
    /// The name of the application to run
    #[clap(short, long, default_value_t = App::default())]
    pub app: App,

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

    /// Enable peer discovery.
    /// If enabled, the node will attempt to discover other nodes in the network
    #[clap(long, default_value = "true")]
    pub enable_discovery: bool,

    /// The transport protocol to use for P2P communication
    /// Possible values:
    /// - "quic": QUIC (default)
    /// - "tcp": TCP + Noise
    #[clap(short, long, default_value = "quic", verbatim_doc_comment)]
    pub transport: TransportProtocol,
}

impl TestnetCmd {
    /// Execute the testnet command
    pub fn run(&self, home_dir: &Path, log_level: LogLevel, log_format: LogFormat) -> Result<()> {
        let node = match self.app {
            App::Starknet => StarknetNode,
        };

        let private_keys = generate_private_keys(&node, self.nodes, self.deterministic);
        let public_keys = private_keys.iter().map(|pk| pk.public_key()).collect();
        let genesis = generate_genesis(&node, public_keys, self.deterministic);

        for (i, private_key) in private_keys.iter().enumerate().take(self.nodes) {
            // Use home directory `home_dir/<index>`
            let node_home_dir = home_dir.join(i.to_string());

            info!(
                id = %i,
                home = %node_home_dir.display(),
                "Generating configuration for node..."
            );

            // Set the destination folder
            let args = Args {
                home: Some(node_home_dir),
                ..Args::default()
            };

            // Save private key
            let priv_validator_key = node.make_private_key_file(*private_key);
            save_priv_validator_key(
                &node,
                &args.get_priv_validator_key_file_path()?,
                &priv_validator_key,
            )?;

            // Save genesis
            save_genesis(&node, &args.get_genesis_file_path()?, &genesis)?;

            // Save config
            save_config(
                &args.get_config_file_path()?,
                &generate_config(
                    self.app,
                    i,
                    self.nodes,
                    self.runtime,
                    self.enable_discovery,
                    self.transport,
                    log_level,
                    log_format,
                ),
            )?;
        }
        Ok(())
    }
}

/// Generate private keys. Random or deterministic for different use-cases.
pub fn generate_private_keys<N>(
    node: &N,
    size: usize,
    deterministic: bool,
) -> Vec<PrivateKey<N::Context>>
where
    N: Node,
{
    if deterministic {
        let mut rng = StdRng::seed_from_u64(0x42);
        (0..size)
            .map(|_| node.generate_private_key(&mut rng))
            .collect()
    } else {
        (0..size)
            .map(|_| node.generate_private_key(OsRng))
            .collect()
    }
}

/// Generate a Genesis file from the public keys and voting power.
/// Voting power can be random or deterministically pseudo-random.
pub fn generate_genesis<N: Node>(
    node: &N,
    pks: Vec<PublicKey<N::Context>>,
    deterministic: bool,
) -> N::Genesis {
    let validators: Vec<_> = if deterministic {
        let mut rng = StdRng::seed_from_u64(0x42);
        pks.into_iter()
            .map(|pk| (pk, rng.gen_range(MIN_VOTING_POWER..=MAX_VOTING_POWER)))
            .collect()
    } else {
        pks.into_iter()
            .map(|pk| (pk, OsRng.gen_range(MIN_VOTING_POWER..=MAX_VOTING_POWER)))
            .collect()
    };

    node.make_genesis(validators)
}

const CONSENSUS_BASE_PORT: usize = 27000;
const MEMPOOL_BASE_PORT: usize = 28000;
const METRICS_BASE_PORT: usize = 29000;

/// Generate configuration for node "index" out of "total" number of nodes.
#[allow(clippy::too_many_arguments)]
pub fn generate_config(
    app: App,
    index: usize,
    total: usize,
    runtime: RuntimeFlavour,
    enable_discovery: bool,
    transport: TransportProtocol,
    log_level: LogLevel,
    log_format: LogFormat,
) -> Config {
    let consensus_port = CONSENSUS_BASE_PORT + index;
    let mempool_port = MEMPOOL_BASE_PORT + index;
    let metrics_port = METRICS_BASE_PORT + index;

    Config {
        app,
        moniker: format!("test-{}", index),
        consensus: ConsensusConfig {
            max_block_size: ByteSize::mib(1),
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                protocol: PubSubProtocol::GossipSub,
                listen_addr: transport.multiaddr("127.0.0.1", consensus_port),
                persistent_peers: if enable_discovery {
                    let mut rng = rand::thread_rng();
                    let count = rng.gen_range(1..=(total / 2));
                    let peers = (0..total)
                        .filter(|j| *j != index)
                        .choose_multiple(&mut rng, count);

                    peers
                        .iter()
                        .unique()
                        .map(|index| transport.multiaddr("127.0.0.1", CONSENSUS_BASE_PORT + index))
                        .collect()
                } else {
                    (0..total)
                        .filter(|j| *j != index)
                        .map(|j| transport.multiaddr("127.0.0.1", CONSENSUS_BASE_PORT + j))
                        .collect()
                },
                discovery: DiscoveryConfig {
                    enabled: enable_discovery,
                },
                transport,
            },
        },
        mempool: MempoolConfig {
            p2p: P2pConfig {
                protocol: PubSubProtocol::GossipSub,
                listen_addr: transport.multiaddr("127.0.0.1", mempool_port),
                persistent_peers: (0..total)
                    .filter(|j| *j != index)
                    .map(|j| transport.multiaddr("127.0.0.1", MEMPOOL_BASE_PORT + j))
                    .collect(),
                discovery: DiscoveryConfig { enabled: true },
                transport,
            },
            max_tx_count: 10000,
            gossip_batch_size: 0,
        },
        metrics: MetricsConfig {
            enabled: true,
            listen_addr: format!("127.0.0.1:{metrics_port}").parse().unwrap(),
        },
        logging: LoggingConfig {
            log_level,
            log_format,
        },
        runtime: match runtime {
            RuntimeFlavour::SingleThreaded => RuntimeConfig::single_threaded(),
            RuntimeFlavour::MultiThreaded(n) => RuntimeConfig::multi_threaded(n),
        },
        test: TestConfig::default(),
    }
}
