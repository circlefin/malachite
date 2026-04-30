#![allow(clippy::too_many_arguments)]

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use rand::{CryptoRng, Rng, RngCore};
use tokio::task::JoinHandle;
use tracing::Instrument;

use malachitebft_app_channel::app::config::*;
use malachitebft_app_channel::app::events::{RxEvent, TxEvent};
use malachitebft_app_channel::app::metrics::SharedRegistry;
use malachitebft_app_channel::app::types::codec::Codec;
use malachitebft_app_channel::app::types::core::VotingPower;
use malachitebft_app_channel::app::types::Keypair;
use malachitebft_app_channel::{
    ConsensusContext, EngineBuilder, EngineHandle, NetworkContext, NetworkIdentity, RequestContext,
    Signer, SyncContext, WalContext,
};
use malachitebft_engine_byzantine::{
    ByzantineMiddleware, ByzantineNetworkProxy, ConflictingValueFn, ConflictingVoteValueFn,
};
use malachitebft_test::codec::proto::ProtobufCodec;
use malachitebft_test::node::{Node, NodeHandle};
use malachitebft_test::traits::{
    CanGeneratePrivateKey, CanMakeConfig, CanMakeGenesis, CanMakePrivateKeyFile, MakeConfigSettings,
};

use malachitebft_test::middleware::{DefaultMiddleware, Middleware};

// Use the same types used for integration tests.
// A real application would use its own types and context instead.
use malachitebft_test::{
    Address, Ed25519Signer, Ed25519Verifier, Genesis, Height, PrivateKey, PublicKey, TestContext,
    Validator, ValidatorSet, Value, ValueId,
};

use crate::config::{Config, ValidatorRotationConfig};
use crate::state::State;
use crate::store::{NoMetrics, Store, StoreMetrics};

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

/// Application struct used by the integration test framework.
/// Configuration and keys are provided in-memory.
#[derive(Clone)]
#[allow(dead_code)]
pub struct App {
    pub home_dir: PathBuf,
    pub config: Config,
    pub validator_set: ValidatorSet,
    pub private_key: PrivateKey,
    pub start_height: Option<Height>,
    pub middleware: Option<Arc<dyn Middleware>>,
    /// When true, the node signs a validator proof and advertises a validator identity.
    /// When false, the node starts without a validator identity.
    pub validator: bool,
}

impl App {
    #[allow(dead_code)]
    fn get_network_keypair(&self) -> Keypair {
        // Separate network identity
        let rng = rand::thread_rng();
        let net_pk = self.generate_private_key(rng);
        Keypair::ed25519_from_bytes(net_pk.inner().to_bytes()).unwrap()
    }
}

#[async_trait]
impl Node for App {
    type Context = TestContext;
    type Config = Config;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKey;
    type Verifier = Ed25519Verifier;
    type Signer = Ed25519Signer;
    type NodeHandle = Handle;

    fn get_home_dir(&self) -> PathBuf {
        self.home_dir.to_owned()
    }

    fn load_config(&self) -> eyre::Result<Self::Config> {
        Ok(self.config.clone())
    }

    fn get_verifier(&self) -> Ed25519Verifier {
        Ed25519Verifier
    }

    fn get_signer(&self, private_key: PrivateKey) -> Ed25519Signer {
        Ed25519Signer::new(private_key)
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

        if let Some(ref byz) = config.byzantine {
            byz.validate()
                .map_err(|e| eyre::eyre!("Invalid byzantine configuration: {e}"))?;
        }

        let public_key = self.get_public_key(&self.private_key);
        let address = self.get_address(&public_key);

        // Wrap middleware with ByzantineMiddleware if amnesia or
        // force-precommit-nil is configured.
        let middleware: Arc<dyn Middleware> = {
            let inner = self
                .middleware
                .clone()
                .unwrap_or_else(|| Arc::new(DefaultMiddleware));

            if let Some(ref byz) = config.byzantine {
                if byz.ignore_locks.is_set() || byz.force_precommit_nil.is_set() {
                    if byz.ignore_locks.is_set() {
                        tracing::warn!(
                            trigger = ?byz.ignore_locks,
                            "BYZANTINE: Amnesia attack enabled (will ignore voting locks)"
                        );
                    }
                    if byz.force_precommit_nil.is_set() {
                        tracing::warn!(
                            trigger = ?byz.force_precommit_nil,
                            "BYZANTINE: Force-precommit-nil enabled (will rewrite non-nil precommits)"
                        );
                    }
                    Arc::new(ByzantineMiddleware::new(
                        byz.ignore_locks.clone(),
                        byz.force_precommit_nil.clone(),
                        inner,
                        address,
                        byz.seed,
                    ))
                } else {
                    inner
                }
            } else {
                inner
            }
        };

        let ctx = TestContext::with_middleware(middleware.clone());
        let keypair = self.get_network_keypair(); // Separate network identity
        let genesis = self.load_genesis()?;
        let wal_path = self.get_home_dir().join("wal").join("consensus.wal");

        let identity = if self.validator {
            let signer = self.get_signer(self.private_key.clone());
            let peer_id_bytes = keypair.public().to_peer_id().to_bytes();
            let proof = signer
                .sign_validator_proof(public_key.as_bytes().to_vec(), peer_id_bytes)
                .await
                .map_err(|e| eyre::eyre!("Failed to sign validator proof: {e:?}"))?;
            let proof_bytes = ProtobufCodec
                .encode(&proof)
                .map_err(|e| eyre::eyre!("Failed to encode validator proof: {e}"))?;
            NetworkIdentity::new_validator(
                config.moniker.clone(),
                keypair,
                address.to_string(),
                proof_bytes,
            )
        } else {
            NetworkIdentity::new(config.moniker.clone(), keypair, None)
        };

        // Build the engine, conditionally injecting the Byzantine proxy
        let builder = EngineBuilder::new(ctx.clone(), config.clone())
            .with_default_wal(WalContext::new(wal_path, ProtobufCodec));

        let is_byzantine = config.byzantine.as_ref().is_some_and(|c| c.is_active());

        let (mut channels, engine_handle) = if is_byzantine {
            let byz_cfg = config.byzantine.clone().unwrap(); // safe: is_active() was true

            tracing::warn!(
                ?byz_cfg,
                "BYZANTINE: Starting node with Byzantine behavior enabled"
            );

            // Spawn the real network actor manually
            let registry = SharedRegistry::global().with_moniker(config.moniker.clone());
            let (real_network, tx_network) = malachitebft_app_channel::spawn::spawn_network_actor(
                identity,
                config.consensus(),
                config.value_sync(),
                &registry,
                ProtobufCodec,
            )
            .await?;

            // Spawn the proxy in front of the real network.
            let conflicting_value_fn: Option<ConflictingValueFn<TestContext>> =
                Some(Box::new(|original: &Value| {
                    Value::new(original.value.wrapping_add(1))
                }));
            let conflicting_vote_value_fn: Option<ConflictingVoteValueFn<TestContext>> =
                Some(Box::new(|original: Option<&ValueId>| match original {
                    Some(id) => ValueId::new(id.as_u64().wrapping_add(1)),
                    None => ValueId::new(0),
                }));

            let proxy_ref = ByzantineNetworkProxy::spawn(
                byz_cfg,
                real_network,
                Box::new(self.get_signer(self.private_key.clone())),
                ctx.clone(),
                address,
                span.clone(),
                conflicting_value_fn,
                conflicting_vote_value_fn,
            )
            .await?;

            builder
                .with_custom_network(proxy_ref, tx_network)
                .with_default_consensus(ConsensusContext::new_validator(
                    address,
                    Box::new(self.get_verifier()),
                    Box::new(self.get_signer(self.private_key.clone())),
                ))
                .with_default_sync(SyncContext::new(ProtobufCodec))
                .with_default_request(RequestContext::new(100))
                .build()
                .await?
        } else {
            let consensus_ctx = if self.validator {
                ConsensusContext::new_validator(
                    address,
                    Box::new(self.get_verifier()),
                    Box::new(self.get_signer(self.private_key.clone())),
                )
            } else {
                ConsensusContext::new_full_node(address, Box::new(self.get_verifier()))
            };

            builder
                .with_default_network(NetworkContext::new(identity, ProtobufCodec))
                .with_default_consensus(consensus_ctx)
                .with_default_sync(SyncContext::new(ProtobufCodec))
                .with_default_request(RequestContext::new(100))
                .build()
                .await?
        };

        drop(_guard);

        let db_path = self.get_home_dir().join("db");
        std::fs::create_dir_all(&db_path)?;

        let store = Store::open(
            db_path.join("store.db"),
            Box::new(NoMetrics) as Box<dyn StoreMetrics>,
        )
        .await?;
        let start_height = self.start_height.unwrap_or_default();

        let mut state = State::new(
            ctx,
            config,
            genesis.clone(),
            address,
            start_height,
            store,
            self.get_signer(self.private_key.clone()),
            Some(middleware),
        );

        let tx_event = channels.events.clone();

        let app_handle = tokio::spawn(
            async move {
                if let Err(e) = crate::app::run(&mut state, &mut channels).await {
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

/// Application struct used by the CLI entry point.
/// Configuration and keys are loaded from files on disk.
#[derive(Clone)]
pub struct CliApp {
    pub home_dir: PathBuf,
    pub config_file: PathBuf,
    pub genesis_file: PathBuf,
    pub private_key_file: PathBuf,
    pub start_height: Option<Height>,
    pub validator: bool,
}

#[async_trait]
impl Node for CliApp {
    type Context = TestContext;
    type Config = Config;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKey;
    type Verifier = Ed25519Verifier;
    type Signer = Ed25519Signer;
    type NodeHandle = Handle;

    fn get_home_dir(&self) -> PathBuf {
        self.home_dir.to_owned()
    }

    fn load_config(&self) -> eyre::Result<Self::Config> {
        crate::config::load_config(&self.config_file, Some("MALACHITE"))
    }

    fn get_verifier(&self) -> Ed25519Verifier {
        Ed25519Verifier
    }

    fn get_signer(&self, private_key: PrivateKey) -> Ed25519Signer {
        Ed25519Signer::new(private_key)
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
        let wal_path = self.get_home_dir().join("wal").join("consensus.wal");
        let ctx = TestContext::new();
        let genesis = self.load_genesis()?;

        // Generate a separate network keypair (distinct from the validator signing key)
        let net_pk = self.generate_private_key(rand::thread_rng());
        let keypair = Keypair::ed25519_from_bytes(net_pk.inner().to_bytes()).unwrap();

        let identity = if self.validator {
            let signer = self.get_signer(private_key.clone());
            let peer_id_bytes = keypair.public().to_peer_id().to_bytes();
            let proof = signer
                .sign_validator_proof(public_key.as_bytes().to_vec(), peer_id_bytes)
                .await
                .map_err(|e| eyre::eyre!("Failed to sign validator proof: {e:?}"))?;
            let proof_bytes = ProtobufCodec
                .encode(&proof)
                .map_err(|e| eyre::eyre!("Failed to encode validator proof: {e}"))?;
            NetworkIdentity::new_validator(
                config.moniker.clone(),
                keypair,
                address.to_string(),
                proof_bytes,
            )
        } else {
            NetworkIdentity::new(config.moniker.clone(), keypair, None)
        };

        let consensus_ctx = if self.validator {
            ConsensusContext::new_validator(
                address,
                Box::new(self.get_verifier()),
                Box::new(self.get_signer(private_key.clone())),
            )
        } else {
            ConsensusContext::new_full_node(address, Box::new(self.get_verifier()))
        };

        let (mut channels, engine_handle) = EngineBuilder::new(ctx.clone(), config.clone())
            .with_default_wal(WalContext::new(wal_path, ProtobufCodec))
            .with_default_network(NetworkContext::new(identity, ProtobufCodec))
            .with_default_consensus(consensus_ctx)
            .with_default_sync(SyncContext::new(ProtobufCodec))
            .with_default_request(RequestContext::new(100))
            .build()
            .await?;

        let tx_event = channels.events.clone();

        let db_dir = self.get_home_dir().join("db");
        std::fs::create_dir_all(&db_dir)?;

        use crate::metrics::DbMetrics;
        use malachitebft_app_channel::app::metrics::SharedRegistry;

        let registry = SharedRegistry::global().with_moniker(&config.moniker);
        let metrics = DbMetrics::register(&registry);

        if config.metrics.enabled {
            use malachitebft_test_cli::metrics;
            tokio::spawn(metrics::serve(config.metrics.listen_addr));
        }

        let store = Store::open(
            db_dir.join("store.db"),
            Box::new(metrics) as Box<dyn StoreMetrics>,
        )
        .await?;
        let start_height = self.start_height.unwrap_or_default();

        let mut state = State::new(
            ctx,
            config.clone(),
            genesis,
            address,
            start_height,
            store,
            Ed25519Signer::new(private_key),
            None,
        );

        let span = tracing::error_span!("node", moniker = %config.moniker);
        let app_handle = tokio::spawn(
            async move {
                if let Err(e) = crate::app::run(&mut state, &mut channels).await {
                    tracing::error!(%e, "Application error");
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

impl CanMakeGenesis for CliApp {
    fn make_genesis(&self, validators: Vec<(PublicKey, VotingPower)>) -> Self::Genesis {
        let validators = validators
            .into_iter()
            .map(|(pk, vp)| Validator::new(pk, vp));

        let validator_set = ValidatorSet::new(validators);

        Genesis { validator_set }
    }
}

impl CanGeneratePrivateKey for CliApp {
    fn generate_private_key<R>(&self, rng: R) -> PrivateKey
    where
        R: RngCore + CryptoRng,
    {
        PrivateKey::generate(rng)
    }
}

impl CanMakePrivateKeyFile for CliApp {
    fn make_private_key_file(&self, private_key: PrivateKey) -> Self::PrivateKeyFile {
        private_key
    }
}

impl CanMakeConfig for CliApp {
    fn make_config(index: usize, total: usize, settings: MakeConfigSettings) -> Self::Config {
        make_config(index, total, settings)
    }
}

/// Generate configuration for node "index" out of "total" number of nodes.
fn make_config(index: usize, total: usize, settings: MakeConfigSettings) -> Config {
    use itertools::Itertools;
    use rand::seq::IteratorRandom;

    const CONSENSUS_BASE_PORT: usize = 27000;
    const METRICS_BASE_PORT: usize = 29000;

    let consensus_port = CONSENSUS_BASE_PORT + index;
    let metrics_port = METRICS_BASE_PORT + index;

    Config {
        moniker: format!("test-{index}"),
        consensus: ConsensusConfig {
            enabled: true,
            value_payload: ValuePayload::ProposalAndParts,
            queue_capacity: 100,
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
                persistent_peers_only: settings.persistent_peers_only,
                ..Default::default()
            },
            ..Default::default()
        },
        metrics: MetricsConfig {
            enabled: true,
            listen_addr: format!("127.0.0.1:{metrics_port}").parse().unwrap(),
        },
        runtime: settings.runtime,
        value_sync: ValueSyncConfig::default(),
        logging: LoggingConfig::default(),
        test: TestConfig::default(),
        byzantine: None,
        validator_rotation: ValidatorRotationConfig::default(),
    }
}
