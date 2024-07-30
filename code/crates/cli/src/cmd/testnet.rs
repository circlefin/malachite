//! Testnet command

use std::path::Path;
use std::str::FromStr;

use bytesize::ByteSize;
use clap::Parser;
use color_eyre::eyre::Result;
use rand::prelude::StdRng;
use rand::rngs::OsRng;
use rand::{Rng, SeedableRng};
use tracing::info;

use malachite_node::config::{
    App, Config, ConsensusConfig, LogFormat, LogLevel, LoggingConfig, MempoolConfig, MetricsConfig,
    P2pConfig, RuntimeConfig, TestConfig, TimeoutConfig,
};
use malachite_test::ValidatorSet as Genesis;
use malachite_test::{PrivateKey, PublicKey, Validator};

use crate::args::Args;
use crate::cmd::init::{save_config, save_genesis, save_priv_validator_key};
use crate::priv_key::PrivValidatorKey;

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
}

impl TestnetCmd {
    /// Execute the testnet command
    pub fn run(&self, home_dir: &Path, log_level: LogLevel, log_format: LogFormat) -> Result<()> {
        let private_keys = generate_private_keys(self.nodes, self.deterministic);
        let public_keys = private_keys.iter().map(|pk| pk.public_key()).collect();
        let genesis = generate_genesis(public_keys, self.deterministic);

        for (i, private_key) in private_keys.iter().enumerate().take(self.nodes) {
            // Use home directory `home_dir/<index>`
            let node_home_dir = home_dir.join(i.to_string());

            info!(
                "Generating configuration for node {i} at `{}`...",
                node_home_dir.display()
            );

            // Set the destination folder
            let args = Args {
                home: Some(node_home_dir),
                ..Args::default()
            };

            // Save private key
            let priv_validator_key = PrivValidatorKey::from(private_key.clone());
            save_priv_validator_key(
                &args.get_priv_validator_key_file_path()?,
                &priv_validator_key,
            )?;

            // Save genesis
            save_genesis(&args.get_genesis_file_path()?, &genesis)?;

            // Save config
            save_config(
                &args.get_config_file_path()?,
                &generate_config(self.app, i, self.nodes, self.runtime, log_level, log_format),
            )?;
        }
        Ok(())
    }
}

/// Generate private keys. Random or deterministic for different use-cases.
pub fn generate_private_keys(size: usize, deterministic: bool) -> Vec<PrivateKey> {
    if deterministic {
        let mut rng = StdRng::seed_from_u64(0x42);
        (0..size).map(|_| PrivateKey::generate(&mut rng)).collect()
    } else {
        (0..size).map(|_| PrivateKey::generate(OsRng)).collect()
    }
}

/// Generate a Genesis file from the public keys and voting power.
/// Voting power can be random or deterministically pseudo-random.
pub fn generate_genesis(pks: Vec<PublicKey>, deterministic: bool) -> Genesis {
    let size = pks.len();
    let voting_powers: Vec<u64> = if deterministic {
        let mut rng = StdRng::seed_from_u64(0x42);
        (0..size)
            .map(|_| rng.gen_range(MIN_VOTING_POWER..=MAX_VOTING_POWER))
            .collect()
    } else {
        (0..size)
            .map(|_| OsRng.gen_range(MIN_VOTING_POWER..=MAX_VOTING_POWER))
            .collect()
    };

    let mut validators = Vec::with_capacity(size);

    for i in 0..size {
        validators.push(Validator::new(pks[i], voting_powers[i]));
    }

    Genesis { validators }
}

const CONSENSUS_BASE_PORT: usize = 27000;
const MEMPOOL_BASE_PORT: usize = 28000;
const METRICS_BASE_PORT: usize = 29000;

/// Generate configuration for node "index" out of "total" number of nodes.
pub fn generate_config(
    app: App,
    index: usize,
    total: usize,
    runtime: RuntimeFlavour,
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
                listen_addr: format!("/ip4/127.0.0.1/udp/{consensus_port}/quic-v1")
                    .parse()
                    .unwrap(),
                persistent_peers: (0..total)
                    .filter(|j| *j != index)
                    .map(|j| {
                        format!("/ip4/127.0.0.1/udp/{}/quic-v1", CONSENSUS_BASE_PORT + j)
                            .parse()
                            .unwrap()
                    })
                    .collect(),
            },
        },
        mempool: MempoolConfig {
            p2p: P2pConfig {
                listen_addr: format!("/ip4/127.0.0.1/udp/{mempool_port}/quic-v1")
                    .parse()
                    .unwrap(),
                persistent_peers: (0..total)
                    .filter(|j| *j != index)
                    .map(|j| {
                        format!("/ip4/127.0.0.1/udp/{}/quic-v1", MEMPOOL_BASE_PORT + j)
                            .parse()
                            .unwrap()
                    })
                    .collect(),
            },
            max_tx_count: 10000,
            gossip_batch_size: 256,
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
