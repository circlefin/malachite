#![allow(clippy::too_many_arguments)]

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use malachitebft_test::middleware::{DefaultMiddleware, Middleware};
use rand::{CryptoRng, RngCore};
use tokio::task::JoinHandle;
use tracing::Instrument;

use malachitebft_app_channel::app::config::*;
use malachitebft_app_channel::app::events::{RxEvent, TxEvent};
use malachitebft_app_channel::app::node::{
    CanGeneratePrivateKey, CanMakeConfig, CanMakeGenesis, CanMakePrivateKeyFile, EngineHandle,
    MakeConfigSettings, Node, NodeHandle,
};
use malachitebft_app_channel::app::types::core::VotingPower;
use malachitebft_app_channel::app::types::Keypair;

// Use the same types used for integration tests.
// A real application would use its own types and context instead.
use malachitebft_test::codec::proto::ProtobufCodec;
use malachitebft_test::{
    Address, Ed25519Provider, Genesis, Height, PrivateKey, PublicKey, TestContext, Validator,
    ValidatorSet,
};

use crate::config::Config;
use crate::state::State;
use crate::store::Store;

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

/// Main application struct implementing the consensus node functionality
#[derive(Clone)]
pub struct App {
    pub home_dir: PathBuf,
    pub config: Config,
    pub validator_set: ValidatorSet,
    pub private_key: PrivateKey,
    pub start_height: Option<Height>,
    pub middleware: Option<Arc<dyn Middleware>>,
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
        Ok(self.config.clone())
    }

    fn get_signing_provider(&self, private_key: PrivateKey) -> Self::SigningProvider {
        Ed25519Provider::new(private_key)
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
        Ok(self.private_key.clone())
    }

    fn load_genesis(&self) -> eyre::Result<Self::Genesis> {
        let validators = self
            .validator_set
            .validators
            .iter()
            .map(|v| (v.public_key, v.voting_power))
            .collect();

        Ok(self.make_genesis(validators))
    }

    async fn start(&self) -> eyre::Result<Handle> {
        let config = self.load_config()?;

        let span = tracing::error_span!("node", moniker = %config.moniker);
        let _guard = span.enter();

        let middleware = self
            .middleware
            .clone()
            .unwrap_or_else(|| Arc::new(DefaultMiddleware));

        let ctx = TestContext::with_middleware(middleware);
        let codec = ProtobufCodec;

        let public_key = self.get_public_key(&self.private_key);
        let address = self.get_address(&public_key);
        let signing_provider = self.get_signing_provider(self.private_key.clone());
        let genesis = self.load_genesis()?;

        let (mut channels, engine_handle) = malachitebft_app_channel::start_engine(
            ctx.clone(),
            codec,
            self.clone(),
            config.clone(),
            self.start_height,
            self.validator_set.clone(),
        )
        .await?;

        drop(_guard);

        let db_path = self.get_home_dir().join("db");
        std::fs::create_dir_all(&db_path)?;

        let store = Store::open(db_path.join("store.db")).await?;
        let start_height = self.start_height.unwrap_or_default();

        let mut state = State::new(
            ctx,
            config,
            genesis.clone(),
            address,
            start_height,
            store,
            signing_provider,
        );

        let tx_event = channels.events.clone();

        let app_handle = tokio::spawn(
            async move {
                if let Err(e) = crate::app::run(genesis, &mut state, &mut channels).await {
                    tracing::error!("Application has failed with an error: {e}");
                }
            }
            .instrument(span),
        );

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
    fn make_config(index: usize, total: usize, settings: MakeConfigSettings) -> Self::Config {
        make_config(index, total, settings)
    }
}

/// Generate configuration for node "index" out of "total" number of nodes.
fn make_config(index: usize, total: usize, settings: MakeConfigSettings) -> Config {
    use itertools::Itertools;
    use rand::seq::IteratorRandom;
    use rand::Rng;

    const CONSENSUS_BASE_PORT: usize = 27000;
    const METRICS_BASE_PORT: usize = 29000;

    let consensus_port = CONSENSUS_BASE_PORT + index;
    let metrics_port = METRICS_BASE_PORT + index;

    Config {
        moniker: format!("test-{}", index),
        consensus: ConsensusConfig {
            // Current test app does not support proposal-only value payload properly as Init does not include valid_round
            value_payload: ValuePayload::ProposalAndParts,
            vote_sync: VoteSyncConfig {
                mode: VoteSyncMode::RequestResponse,
            },
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
