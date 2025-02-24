//! The Application (or Node) definition. The Node trait implements the Consensus context and the
//! cryptographic library used for signing.

use std::path::PathBuf;

use async_trait::async_trait;
use malachitebft_app_channel::app::events::{RxEvent, TxEvent};
use malachitebft_test_cli::config::{
    BootstrapProtocol, RuntimeConfig, Selector, TransportProtocol,
};
use rand::{CryptoRng, RngCore};

use malachitebft_app_channel::app::metrics::SharedRegistry;
use malachitebft_app_channel::app::types::core::{Height as _, VotingPower};
use malachitebft_app_channel::app::types::Keypair;
use malachitebft_app_channel::app::{
    CanGeneratePrivateKey, CanMakeConfig, CanMakeGenesis, CanMakePrivateKeyFile, EngineHandle,
    Node, NodeHandle,
};

// Use the same types used for integration tests.
// A real application would use its own types and context instead.
use malachitebft_test::codec::proto::ProtobufCodec;
use malachitebft_test::{
    Address, Ed25519Provider, Genesis, Height, PrivateKey, PublicKey, TestContext, Validator,
    ValidatorSet,
};
use malachitebft_test_cli::metrics;
use tokio::task::JoinHandle;

use crate::config::{load_config, Config};
use crate::metrics::DbMetrics;
use crate::state::State;
use crate::store::Store;

/// Main application struct implementing the consensus node functionality
#[derive(Clone)]
pub struct App {
    pub home_dir: PathBuf,
    pub config_file: PathBuf,
    pub genesis_file: PathBuf,
    pub private_key_file: PathBuf,
    pub start_height: Option<Height>,
}

pub struct Handle {
    pub app: JoinHandle<()>,
    pub engine: EngineHandle,
    pub tx_event: TxEvent<TestContext>,
}

#[async_trait]
impl NodeHandle<TestContext> for Handle {
    fn subscribe(&self) -> RxEvent<TestContext> {
        self.tx_event.subscribe()
    }

    async fn kill(&self, _reason: Option<String>) -> eyre::Result<()> {
        self.engine.actor.kill_and_wait(None).await?;
        self.app.abort();
        self.engine.handle.abort();
        Ok(())
    }
}

#[async_trait]
impl Node for App {
    type Context = TestContext;
    type Config = Config;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKey;
    type SigningProvider = Ed25519Provider;
    type NodeHandle = Handle;

    fn get_home_dir(&self) -> PathBuf {
        self.home_dir.to_owned()
    }

    fn load_config(&self) -> eyre::Result<Self::Config> {
        load_config(&self.config_file, Some("MALACHITE"))
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
        file
    }

    fn load_private_key_file(&self) -> eyre::Result<Self::PrivateKeyFile> {
        let private_key = std::fs::read_to_string(&self.private_key_file)?;
        serde_json::from_str(&private_key).map_err(Into::into)
    }

    fn get_signing_provider(&self, private_key: PrivateKey) -> Self::SigningProvider {
        Ed25519Provider::new(private_key)
    }

    fn load_genesis(&self) -> eyre::Result<Self::Genesis> {
        let genesis = std::fs::read_to_string(&self.genesis_file)?;
        serde_json::from_str(&genesis).map_err(Into::into)
    }

    async fn start(&self) -> eyre::Result<Handle> {
        let config = self.load_config()?;

        let span = tracing::error_span!("node", moniker = %config.moniker);
        let _enter = span.enter();

        let private_key_file = self.load_private_key_file()?;
        let private_key = self.load_private_key(private_key_file);
        let public_key = self.get_public_key(&private_key);
        let address = self.get_address(&public_key);
        let signing_provider = self.get_signing_provider(private_key);
        let ctx = TestContext::new();

        let genesis = self.load_genesis()?;
        let initial_validator_set = genesis.validator_set.clone();

        let codec = ProtobufCodec;

        let (mut channels, engine_handle) = malachitebft_app_channel::start_engine(
            ctx,
            codec,
            self.clone(),
            config.clone(),
            self.start_height,
            initial_validator_set,
        )
        .await?;

        let tx_event = channels.events.clone();

        let registry = SharedRegistry::global().with_moniker(&config.moniker);
        let metrics = DbMetrics::register(&registry);

        if config.metrics.enabled {
            tokio::spawn(metrics::serve(config.metrics.listen_addr));
        }

        let db_dir = self.get_home_dir().join("db");
        std::fs::create_dir_all(&db_dir)?;

        let store = Store::open(db_dir.join("store.db"), metrics)?;
        let start_height = self.start_height.unwrap_or(Height::INITIAL);
        let mut state = State::new(ctx, signing_provider, genesis, address, start_height, store);

        let app_handle = tokio::spawn(async move {
            if let Err(e) = crate::app::run(&mut state, &mut channels).await {
                tracing::error!(%e, "Application error");
            }
        });

        Ok(Handle {
            app: app_handle,
            engine: engine_handle,
            tx_event,
        })
    }

    async fn run(self) -> eyre::Result<()> {
        let handles = self.start().await?;
        handles.app.await.map_err(Into::into)
    }
}

impl CanMakeGenesis for App {
    fn make_genesis(&self, validators: Vec<(PublicKey, VotingPower)>) -> Self::Genesis {
        let validators = validators
            .into_iter()
            .map(|(pk, vp)| Validator::new(pk, vp));

        let validator_set = ValidatorSet::new(validators);

        Genesis { validator_set }
    }
}

impl CanGeneratePrivateKey for App {
    fn generate_private_key<R>(&self, rng: R) -> PrivateKey
    where
        R: RngCore + CryptoRng,
    {
        PrivateKey::generate(rng)
    }
}

impl CanMakePrivateKeyFile for App {
    fn make_private_key_file(&self, private_key: PrivateKey) -> Self::PrivateKeyFile {
        private_key
    }
}

impl CanMakeConfig for App {
    fn make_config(
        index: usize,
        total: usize,
        runtime: RuntimeConfig,
        enable_discovery: bool,
        bootstrap_protocol: BootstrapProtocol,
        selector: Selector,
        num_outbound_peers: usize,
        num_inbound_peers: usize,
        ephemeral_connection_timeout_ms: u64,
        transport: TransportProtocol,
    ) -> Self::Config {
        make_config(
            index,
            total,
            runtime,
            enable_discovery,
            bootstrap_protocol,
            selector,
            num_outbound_peers,
            num_inbound_peers,
            ephemeral_connection_timeout_ms,
            transport,
        )
    }
}

/// Generate configuration for node "index" out of "total" number of nodes.
#[allow(clippy::too_many_arguments)]
fn make_config(
    index: usize,
    total: usize,
    runtime: RuntimeConfig,
    enable_discovery: bool,
    bootstrap_protocol: BootstrapProtocol,
    selector: Selector,
    num_outbound_peers: usize,
    num_inbound_peers: usize,
    ephemeral_connection_timeout_ms: u64,
    transport: TransportProtocol,
) -> Config {
    use itertools::Itertools;
    use rand::seq::IteratorRandom;
    use rand::Rng;
    use std::time::Duration;

    use malachitebft_app_channel::app::config::*;

    const CONSENSUS_BASE_PORT: usize = 27000;
    const METRICS_BASE_PORT: usize = 29000;

    let consensus_port = CONSENSUS_BASE_PORT + index;
    let metrics_port = METRICS_BASE_PORT + index;

    Config {
        moniker: format!("app-{}", index),
        consensus: ConsensusConfig {
            value_payload: ValuePayload::PartsOnly,
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                protocol: PubSubProtocol::default(),
                listen_addr: transport.multiaddr("127.0.0.1", consensus_port),
                persistent_peers: if enable_discovery {
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
        sync: Default::default(),
        metrics: MetricsConfig {
            enabled: true,
            listen_addr: format!("127.0.0.1:{metrics_port}").parse().unwrap(),
        },
        runtime,
        logging: LoggingConfig::default(),
    }
}
