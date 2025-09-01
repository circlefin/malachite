#![allow(clippy::too_many_arguments)]

use std::path::PathBuf;

use ractor::async_trait;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use malachitebft_app::events::{RxEvent, TxEvent};
use malachitebft_app::node::{
    CanGeneratePrivateKey, CanMakeConfig, CanMakeDistributedConfig, CanMakeGenesis,
    CanMakePrivateKeyFile, MakeConfigSettings, Node, NodeHandle,
};
use malachitebft_app::types::Keypair;
use malachitebft_config::{mempool_load::UniformLoadConfig, ValueSyncConfig};
use malachitebft_core_types::VotingPower;
use malachitebft_engine::node::NodeRef;

use crate::config::{load_config, Config};
use crate::spawn::spawn_node_actor;

use crate::types::{
    Address, Ed25519Provider, Height, MockContext, PrivateKey, PublicKey, Validator, ValidatorSet,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Genesis {
    pub validator_set: ValidatorSet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivateKeyFile {
    pub private_key: PrivateKey,
    pub public_key: PublicKey,
    pub address: Address,
}

impl From<PrivateKey> for PrivateKeyFile {
    fn from(private_key: PrivateKey) -> Self {
        let public_key = private_key.public_key();
        let address = Address::from_public_key(&public_key);

        Self {
            private_key,
            public_key,
            address,
        }
    }
}

pub struct Handle {
    pub actor: NodeRef,
    pub handle: JoinHandle<()>,
    pub tx_event: TxEvent<MockContext>,
}

#[async_trait]
impl NodeHandle<MockContext> for Handle {
    fn subscribe(&self) -> RxEvent<MockContext> {
        self.tx_event.subscribe()
    }

    async fn kill(&self, _reason: Option<String>) -> eyre::Result<()> {
        self.actor.kill_and_wait(None).await?;
        self.handle.abort();
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum ConfigSource {
    File(PathBuf),
    Value(Box<Config>),
    Default,
}

#[derive(Clone, Debug)]
pub struct ActorNode {
    pub home_dir: PathBuf,
    pub config_source: ConfigSource,
    pub start_height: Option<u64>,
}

impl ActorNode {
    pub fn new(home_dir: PathBuf, config_source: ConfigSource, start_height: Option<u64>) -> Self {
        Self {
            home_dir,
            config_source,
            start_height,
        }
    }

    pub fn genesis_file(&self) -> PathBuf {
        self.home_dir.join("config").join("genesis.json")
    }

    pub fn private_key_file(&self) -> PathBuf {
        self.home_dir.join("config").join("priv_validator_key.json")
    }
}

#[async_trait]
impl Node for ActorNode {
    type Context = MockContext;
    type Config = Config;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKeyFile;
    type SigningProvider = Ed25519Provider;
    type NodeHandle = Handle;

    fn get_home_dir(&self) -> PathBuf {
        self.home_dir.to_owned()
    }

    fn load_config(&self) -> eyre::Result<Self::Config> {
        match self.config_source {
            ConfigSource::File(ref path) => load_config(path, Some("MALACHITE")),
            ConfigSource::Value(ref config) => Ok(*config.clone()),
            ConfigSource::Default => Ok(default_config()),
        }
    }

    fn get_address(&self, pk: &PublicKey) -> Address {
        Address::from_public_key(pk)
    }

    fn get_public_key(&self, pk: &PrivateKey) -> PublicKey {
        pk.public_key()
    }

    fn get_keypair(&self, pk: PrivateKey) -> Keypair {
        Keypair::ed25519_from_bytes(pk.inner().to_bytes()).unwrap()
    }

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey {
        file.private_key
    }

    fn load_private_key_file(&self) -> eyre::Result<Self::PrivateKeyFile> {
        let private_key = std::fs::read_to_string(self.private_key_file())?;
        serde_json::from_str(&private_key).map_err(|e| e.into())
    }

    fn get_signing_provider(&self, private_key: PrivateKey) -> Self::SigningProvider {
        Self::SigningProvider::new(private_key)
    }

    fn load_genesis(&self) -> eyre::Result<Self::Genesis> {
        let genesis = std::fs::read_to_string(self.genesis_file())?;
        serde_json::from_str(&genesis).map_err(|e| e.into())
    }

    async fn start(&self) -> eyre::Result<Handle> {
        let config = self.load_config()?;

        let span = tracing::error_span!("node", moniker = %config.moniker);
        let _enter = span.enter();

        let priv_key_file = self.load_private_key_file()?;
        let private_key = self.load_private_key(priv_key_file);
        let genesis = self.load_genesis()?;

        let tx_event = TxEvent::new();

        let start_height = self.start_height.map(Height::new);

        let (actor, handle) = spawn_node_actor(
            config.clone(),
            self.home_dir.clone(),
            genesis.validator_set,
            private_key,
            start_height,
            tx_event.clone(),
            span.clone(),
        )
        .await;

        Ok(Handle {
            actor,
            handle,
            tx_event,
        })
    }

    async fn run(self) -> eyre::Result<()> {
        let handle = self.start().await?;
        handle.actor.wait(None).await.map_err(Into::into)
    }
}

impl CanGeneratePrivateKey for ActorNode {
    fn generate_private_key<R>(&self, rng: R) -> PrivateKey
    where
        R: RngCore + CryptoRng,
    {
        PrivateKey::generate(rng)
    }
}

impl CanMakePrivateKeyFile for ActorNode {
    fn make_private_key_file(&self, private_key: PrivateKey) -> Self::PrivateKeyFile {
        PrivateKeyFile::from(private_key)
    }
}

impl CanMakeGenesis for ActorNode {
    fn make_genesis(&self, validators: Vec<(PublicKey, VotingPower)>) -> Self::Genesis {
        let validators = validators
            .into_iter()
            .map(|(pk, vp)| Validator::new(pk, vp));

        let validator_set = ValidatorSet::new(validators);

        Genesis { validator_set }
    }
}

impl CanMakeConfig for ActorNode {
    fn make_config(index: usize, total: usize, settings: MakeConfigSettings) -> Self::Config {
        make_config(index, total, settings)
    }
}

impl CanMakeDistributedConfig for ActorNode {
    fn make_distributed_config(
        index: usize,
        total: usize,
        machines: Vec<String>,
        bootstrap_set_size: usize,
        settings: MakeConfigSettings,
    ) -> Self::Config {
        make_distributed_config(index, total, machines, bootstrap_set_size, settings)
    }
}

/// Generate configuration for node "index" out of "total" number of nodes.
fn make_config(index: usize, total: usize, settings: MakeConfigSettings) -> Config {
    use itertools::Itertools;
    use rand::seq::IteratorRandom;
    use rand::Rng;

    use malachitebft_config::*;

    const CONSENSUS_BASE_PORT: usize = 27000;
    const MEMPOOL_BASE_PORT: usize = 28000;
    const METRICS_BASE_PORT: usize = 29000;

    let consensus_port = CONSENSUS_BASE_PORT + index;
    let mempool_port = MEMPOOL_BASE_PORT + index;
    let metrics_port = METRICS_BASE_PORT + index;

    Config {
        moniker: format!("actor-app-proposal-{index}"),
        consensus: ConsensusConfig {
            queue_capacity: 1000,
            value_payload: ValuePayload::ProposalOnly,
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                protocol: PubSubProtocol::default(),
                listen_addr: settings.transport.multiaddr("127.0.0.1", consensus_port),
                persistent_peers: if settings.discovery.enabled {
                    let mut rng = rand::thread_rng();
                    let count = if total > 1 {
                        rng.gen_range(1..=(total / 2))
                    } else {
                        0
                    };
                    let peers = (0..total)
                        .filter(|j| *j != index)
                        .choose_multiple(&mut rng, count);

                    peers
                        .iter()
                        .unique()
                        .map(|index| {
                            settings
                                .transport
                                .multiaddr("127.0.0.1", CONSENSUS_BASE_PORT + index)
                        })
                        .collect()
                } else {
                    (0..total)
                        .filter(|j| *j != index)
                        .map(|j| {
                            settings
                                .transport
                                .multiaddr("127.0.0.1", CONSENSUS_BASE_PORT + j)
                        })
                        .collect()
                },
                discovery: settings.discovery,
                ..Default::default()
            },
        },
        mempool: MempoolConfig {
            p2p: P2pConfig {
                protocol: PubSubProtocol::default(),
                listen_addr: settings.transport.multiaddr("127.0.0.1", mempool_port),
                persistent_peers: (0..total)
                    .filter(|j| *j != index)
                    .map(|j| {
                        settings
                            .transport
                            .multiaddr("127.0.0.1", MEMPOOL_BASE_PORT + j)
                    })
                    .collect(),
                discovery: DiscoveryConfig {
                    enabled: false,
                    ..settings.discovery
                },
                ..Default::default()
            },
            max_tx_count: 10000,
            gossip_batch_size: 0,
            load: MempoolLoadConfig {
                load_type: MempoolLoadType::UniformLoad(UniformLoadConfig::default()),
            },
        },
        metrics: MetricsConfig {
            enabled: true,
            listen_addr: format!("127.0.0.1:{metrics_port}").parse().unwrap(),
        },
        runtime: settings.runtime,
        value_sync: ValueSyncConfig::default(),
        logging: LoggingConfig::default(),
        test: TestConfig::default(),
    }
}

fn make_distributed_config(
    index: usize,
    _total: usize,
    machines: Vec<String>,
    bootstrap_set_size: usize,
    settings: MakeConfigSettings,
) -> Config {
    use itertools::Itertools;
    use malachitebft_config::*;
    use std::time::Duration;

    const CONSENSUS_BASE_PORT: usize = 27000;
    const MEMPOOL_BASE_PORT: usize = 28000;
    const METRICS_BASE_PORT: usize = 29000;

    let machine = machines[index % machines.len()].clone();
    let consensus_port = CONSENSUS_BASE_PORT + (index / machines.len());
    let mempool_port = MEMPOOL_BASE_PORT + (index / machines.len());
    let metrics_port = METRICS_BASE_PORT + (index / machines.len());

    Config {
        moniker: format!("actor-app-proposal-{index}"),
        consensus: ConsensusConfig {
            queue_capacity: 1000,
            value_payload: ValuePayload::ProposalOnly,
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                protocol: PubSubProtocol::default(),
                listen_addr: settings.transport.multiaddr(&machine, consensus_port),
                persistent_peers: if settings.discovery.enabled {
                    let peers =
                        ((index.saturating_sub(bootstrap_set_size))..index).collect::<Vec<_>>();

                    peers
                        .iter()
                        .unique()
                        .map(|j| {
                            settings.transport.multiaddr(
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
                            settings.transport.multiaddr(
                                &machines[*j % machines.len()],
                                CONSENSUS_BASE_PORT + (*j / machines.len()),
                            )
                        })
                        .collect()
                },
                discovery: settings.discovery,
                ..Default::default()
            },
        },
        mempool: MempoolConfig {
            p2p: P2pConfig {
                protocol: PubSubProtocol::default(),
                listen_addr: settings.transport.multiaddr(&machine, mempool_port),
                persistent_peers: vec![],
                discovery: DiscoveryConfig {
                    enabled: false,
                    ..DiscoveryConfig::default()
                },
                ..Default::default()
            },
            max_tx_count: 10000,
            gossip_batch_size: 0,
            load: MempoolLoadConfig {
                load_type: MempoolLoadType::UniformLoad(UniformLoadConfig::default()),
            },
        },
        value_sync: ValueSyncConfig {
            enabled: false,
            status_update_interval: Duration::from_secs(0),
            request_timeout: Duration::from_secs(0),
            ..Default::default()
        },
        metrics: MetricsConfig {
            enabled: true,
            listen_addr: format!("{machine}:{metrics_port}").parse().unwrap(),
        },
        runtime: settings.runtime,
        logging: LoggingConfig::default(),
        test: TestConfig::default(),
    }
}

fn default_config() -> Config {
    use malachitebft_config::{DiscoveryConfig, RuntimeConfig, TransportProtocol};

    make_config(
        1,
        3,
        MakeConfigSettings {
            runtime: RuntimeConfig::single_threaded(),
            transport: TransportProtocol::Tcp,
            discovery: DiscoveryConfig::default(),
            value_sync: ValueSyncConfig::default(),
        },
    )
}

#[test]
fn test_actor_node() {
    // Create temp folder for configuration files
    let temp_dir =
        tempfile::TempDir::with_prefix("proposal-only-node-").expect("Failed to create temp dir");

    let temp_path = temp_dir.path().to_owned();

    if std::env::var("KEEP_TEMP").is_ok() {
        std::mem::forget(temp_dir);
    }

    std::fs::create_dir_all(temp_path.join("config")).unwrap();

    // Create default configuration
    let node = ActorNode::new(temp_path.clone(), ConfigSource::Default, Some(1));

    // Create configuration files
    use malachitebft_test_cli::*;

    let priv_keys = new::generate_private_keys(&node, 1, true);
    let pub_keys = priv_keys.iter().map(|pk| node.get_public_key(pk)).collect();
    let genesis = new::generate_genesis(&node, pub_keys, true);

    file::save_priv_validator_key(
        &node,
        &node.private_key_file(),
        &PrivateKeyFile::from(priv_keys[0].clone()),
    )
    .unwrap();

    file::save_genesis(&node, &node.genesis_file(), &genesis).unwrap();

    let config = node.load_config().unwrap();

    // Run the node for a few seconds
    const TIMEOUT: u64 = 3;
    use tokio::time::{timeout, Duration};
    let rt = malachitebft_test_cli::runtime::build_runtime(config.runtime).unwrap();
    let result = rt.block_on(async { timeout(Duration::from_secs(TIMEOUT), node.run()).await });

    // Check that the node did not quit before the timeout.
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "deadline has elapsed");
    let io_error: std::io::Error = error.into();
    assert_eq!(
        io_error.to_string(),
        std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out").to_string()
    );
}
