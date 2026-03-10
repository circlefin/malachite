//! Builder pattern for constructing the consensus engine with optional custom actors.
//!
//! This module provides a type-safe builder that uses const generics to track
//! at compile-time which actors have been configured. The `build()` method is
//! only available when all required actors have been configured.

use std::path::PathBuf;
use std::sync::Arc;

use eyre::Result;
use tokio::sync::mpsc::{self, Sender};

use malachitebft_app::types::codec::HasEncodedLen;
use malachitebft_engine::network::{NetworkIdentity, NetworkRef};
use malachitebft_engine::sync::SyncRef;
use malachitebft_engine::util::events::TxEvent;
use malachitebft_engine::util::output_port::{OutputPort, OutputPortSubscriberTrait};
use malachitebft_engine::wal::WalRef;
use malachitebft_signing::SigningProvider;

use crate::app::config::NodeConfig;
use crate::app::metrics::{Metrics, SharedRegistry};
use crate::app::spawn::{
    spawn_consensus_actor, spawn_node_actor, spawn_sync_actor, spawn_wal_actor,
};
use crate::app::types::codec;
use crate::app::types::core::Context;
use crate::msgs::NetworkMsg;
use crate::spawn::{spawn_host_actor, spawn_network_actor};
use crate::{Channels, EngineHandle};

pub enum NoCodec {}

impl<T> codec::Codec<T> for NoCodec {
    type Error = std::convert::Infallible;

    fn decode(&self, _: bytes::Bytes) -> std::result::Result<T, Self::Error> {
        unreachable!()
    }

    fn encode(&self, _: &T) -> std::result::Result<bytes::Bytes, Self::Error> {
        unreachable!()
    }
}

impl<T> HasEncodedLen<T> for NoCodec {
    fn encoded_len(&self, _: &T) -> Result<usize, Self::Error> {
        unreachable!()
    }
}

/// Context for spawning the WAL actor.
pub struct WalContext<Codec> {
    pub path: PathBuf,
    pub codec: Codec,
}

impl<Codec> WalContext<Codec> {
    pub fn new(path: PathBuf, codec: Codec) -> Self {
        Self { path, codec }
    }
}

/// Context for spawning the Network actor.
pub struct NetworkContext<Codec> {
    pub identity: NetworkIdentity,
    pub codec: Codec,
}

impl<Codec> NetworkContext<Codec> {
    pub fn new(identity: NetworkIdentity, codec: Codec) -> Self {
        Self { identity, codec }
    }
}

/// Context for spawning the Consensus actor.
pub struct ConsensusContext<Ctx: Context, Signer> {
    pub address: Ctx::Address,
    pub signing_provider: Signer,
}

impl<Ctx: Context, Signer> ConsensusContext<Ctx, Signer> {
    pub fn new(address: Ctx::Address, signing_provider: Signer) -> Self {
        Self {
            address,
            signing_provider,
        }
    }
}

/// Context for spawning the Sync actor.
pub struct SyncContext<Codec> {
    pub codec: Codec,
}

impl<Codec> SyncContext<Codec> {
    pub fn new(codec: Codec) -> Self {
        Self { codec }
    }
}

/// Context for request channels.
pub struct RequestContext {
    pub channel_size: usize,
}

impl RequestContext {
    pub fn new(channel_size: usize) -> Self {
        Self { channel_size }
    }
}

/// Builder for the WAL actor - either default or custom.
pub enum WalBuilder<Ctx: Context, Codec> {
    /// Use the default WAL actor with the given context.
    Default(WalContext<Codec>),
    /// Use a custom WAL actor reference.
    Custom(WalRef<Ctx>),
}

/// Builder for the Network actor - either default or custom.
#[allow(clippy::large_enum_variant)]
pub enum NetworkBuilder<Ctx: Context, Codec> {
    /// Use the default Network actor with the given context.
    Default(NetworkContext<Codec>),
    /// Use a custom Network actor reference and message sender.
    Custom((NetworkRef<Ctx>, Sender<NetworkMsg<Ctx>>)),
}

/// Builder for the Sync actor - either default, custom, or disabled.
pub enum SyncBuilder<Ctx: Context, Codec> {
    /// Use the default Sync actor with the given context.
    Default(SyncContext<Codec>),
    /// Use a custom Sync actor reference, or `None` to disable sync.
    Custom(Option<SyncRef<Ctx>>),
}

/// Builder for the Consensus actor.
pub enum ConsensusBuilder<Ctx: Context, Signer> {
    /// Use the default Consensus actor with the given context.
    Default(ConsensusContext<Ctx, Signer>),
}

/// Builder for request channels.
pub enum RequestBuilder {
    /// Use the default request channel configuration.
    Default(RequestContext),
}

/// Builder for constructing the consensus engine with optional custom actors.
///
/// This builder uses const generics to track at compile-time which actors have been
/// configured. The `build()` method is only available when all required actors
/// (WAL, Network, Sync, Consensus, Request) have been configured.
///
/// This builder allows you to:
/// - Use all default actors (simplest case)
/// - Replace specific actors with custom implementations
/// - Mix and match default and custom actors
///
/// # Example: All defaults
/// ```rust,ignore
/// // Sign the validator proof (ADR-006) and build the network identity
/// let proof = signer.sign_validator_proof(public_key_bytes, peer_id_bytes).await?;
/// let proof_bytes = codec.encode(&proof)?;
/// let identity = NetworkIdentity::new_validator(moniker, keypair, address, proof_bytes);
///
/// let (channels, handle) = EngineBuilder::new(ctx, config)
///     .with_default_wal(WalContext::new(path, codec))
///     .with_default_network(NetworkContext::new(identity, codec))
///     .with_default_sync(SyncContext::new(sync_codec))
///     .with_default_consensus(ConsensusContext::new(address, signer))
///     .with_default_request(RequestContext::new(100))
///     .build()
///     .await?;
/// ```
///
/// # Example: Custom network actor
/// ```rust,ignore
/// let (network_ref, tx_network) = spawn_custom_network_actor().await?;
///
/// let (channels, handle) = EngineBuilder::new(ctx, config)
///     .with_default_wal(WalContext::new(path, codec))
///     .with_custom_network(network_ref, tx_network)
///     .with_default_sync(SyncContext::new(sync_codec))
///     .with_default_consensus(ConsensusContext::new(address, signer))
///     .with_default_request(RequestContext::new(100))
///     .build()
///     .await?;
/// ```
pub struct EngineBuilder<
    Ctx,
    Config,
    Signer,
    WalCodec,
    NetCodec,
    SyncCodec,
    const HAS_WAL: bool = false,
    const HAS_NETWORK: bool = false,
    const HAS_SYNC: bool = false,
    const HAS_CONSENSUS: bool = false,
    const HAS_REQUEST: bool = false,
> where
    Ctx: Context,
{
    // Required context parameters
    ctx: Ctx,
    config: Config,

    // Actor builders (stored as enums that hold either default context or custom actor)
    wal: Option<WalBuilder<Ctx, WalCodec>>,
    network: Option<NetworkBuilder<Ctx, NetCodec>>,
    sync: Option<SyncBuilder<Ctx, SyncCodec>>,
    consensus: Option<ConsensusBuilder<Ctx, Signer>>,
    request: Option<RequestBuilder>,
}

// Implementation for creating a new builder (all flags start as false, codec types default to NoCodec)
impl<Ctx, Config, Signer> EngineBuilder<Ctx, Config, Signer, NoCodec, NoCodec, NoCodec>
where
    Ctx: Context,
{
    /// Create a new engine builder with the required context and configuration.
    ///
    /// All actor configurations start unconfigured. You must configure all required
    /// actors (WAL, Network, Sync, Consensus, Request) before `build()` becomes available.
    ///
    /// The codec type parameters default to `NoCodec` and will be inferred based on
    /// the methods used to configure each actor:
    /// - `with_default_*` methods will set the codec type based on the context provided
    /// - `with_custom_*` methods keep the codec type as `NoCodec`
    pub fn new(ctx: Ctx, config: Config) -> Self {
        Self {
            ctx,
            config,
            wal: None,
            network: None,
            sync: None,
            consensus: None,
            request: None,
        }
    }
}

// Implementation for configuration methods (available on any builder state)
impl<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        SyncCodec,
        const HAS_WAL: bool,
        const HAS_NETWORK: bool,
        const HAS_SYNC: bool,
        const HAS_CONSENSUS: bool,
        const HAS_REQUEST: bool,
    >
    EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        SyncCodec,
        HAS_WAL,
        HAS_NETWORK,
        HAS_SYNC,
        HAS_CONSENSUS,
        HAS_REQUEST,
    >
where
    Ctx: Context,
{
    /// Use the default Consensus actor with the given context.
    #[must_use]
    pub fn with_default_consensus(
        self,
        context: ConsensusContext<Ctx, Signer>,
    ) -> EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        SyncCodec,
        HAS_WAL,
        HAS_NETWORK,
        HAS_SYNC,
        true,
        HAS_REQUEST,
    > {
        EngineBuilder {
            ctx: self.ctx,
            config: self.config,
            wal: self.wal,
            network: self.network,
            sync: self.sync,
            consensus: Some(ConsensusBuilder::Default(context)),
            request: self.request,
        }
    }

    /// Use the default request channel configuration.
    #[must_use]
    pub fn with_default_request(
        self,
        context: RequestContext,
    ) -> EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        SyncCodec,
        HAS_WAL,
        HAS_NETWORK,
        HAS_SYNC,
        HAS_CONSENSUS,
        true,
    > {
        EngineBuilder {
            ctx: self.ctx,
            config: self.config,
            wal: self.wal,
            network: self.network,
            sync: self.sync,
            consensus: self.consensus,
            request: Some(RequestBuilder::Default(context)),
        }
    }
}

// Implementation for default WAL actor (allows changing the WalCodec type)
impl<
        Ctx,
        Config,
        Signer,
        NetCodec,
        SyncCodec,
        const HAS_NETWORK: bool,
        const HAS_SYNC: bool,
        const HAS_CONSENSUS: bool,
        const HAS_REQUEST: bool,
    >
    EngineBuilder<
        Ctx,
        Config,
        Signer,
        NoCodec,
        NetCodec,
        SyncCodec,
        false,
        HAS_NETWORK,
        HAS_SYNC,
        HAS_CONSENSUS,
        HAS_REQUEST,
    >
where
    Ctx: Context,
{
    /// Use the default WAL actor with the given context.
    #[must_use]
    pub fn with_default_wal<WalCodec>(
        self,
        context: WalContext<WalCodec>,
    ) -> EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        SyncCodec,
        true,
        HAS_NETWORK,
        HAS_SYNC,
        HAS_CONSENSUS,
        HAS_REQUEST,
    > {
        EngineBuilder {
            ctx: self.ctx,
            config: self.config,
            wal: Some(WalBuilder::Default(context)),
            network: self.network,
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }
}

// Implementation for default Network actor (allows changing the NetCodec type)
impl<
        Ctx,
        Config,
        Signer,
        WalCodec,
        SyncCodec,
        const HAS_WAL: bool,
        const HAS_SYNC: bool,
        const HAS_CONSENSUS: bool,
        const HAS_REQUEST: bool,
    >
    EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NoCodec,
        SyncCodec,
        HAS_WAL,
        false,
        HAS_SYNC,
        HAS_CONSENSUS,
        HAS_REQUEST,
    >
where
    Ctx: Context,
{
    /// Use the default Network actor with the given context.
    #[must_use]
    pub fn with_default_network<NetCodec>(
        self,
        context: NetworkContext<NetCodec>,
    ) -> EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        SyncCodec,
        HAS_WAL,
        true,
        HAS_SYNC,
        HAS_CONSENSUS,
        HAS_REQUEST,
    > {
        EngineBuilder {
            ctx: self.ctx,
            config: self.config,
            wal: self.wal,
            network: Some(NetworkBuilder::Default(context)),
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }
}

// Implementation for default Sync actor (allows changing the SyncCodec type)
impl<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        const HAS_WAL: bool,
        const HAS_NETWORK: bool,
        const HAS_CONSENSUS: bool,
        const HAS_REQUEST: bool,
    >
    EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        NoCodec,
        HAS_WAL,
        HAS_NETWORK,
        false,
        HAS_CONSENSUS,
        HAS_REQUEST,
    >
where
    Ctx: Context,
{
    /// Use the default Sync actor with the given context.
    #[must_use]
    pub fn with_default_sync<SyncCodec>(
        self,
        context: SyncContext<SyncCodec>,
    ) -> EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        SyncCodec,
        HAS_WAL,
        HAS_NETWORK,
        true,
        HAS_CONSENSUS,
        HAS_REQUEST,
    > {
        EngineBuilder {
            ctx: self.ctx,
            config: self.config,
            wal: self.wal,
            network: self.network,
            sync: Some(SyncBuilder::Default(context)),
            consensus: self.consensus,
            request: self.request,
        }
    }
}

// Implementation for custom WAL actor
impl<
        Ctx,
        Config,
        Signer,
        NetCodec,
        SyncCodec,
        const HAS_NETWORK: bool,
        const HAS_SYNC: bool,
        const HAS_CONSENSUS: bool,
        const HAS_REQUEST: bool,
    >
    EngineBuilder<
        Ctx,
        Config,
        Signer,
        NoCodec,
        NetCodec,
        SyncCodec,
        false,
        HAS_NETWORK,
        HAS_SYNC,
        HAS_CONSENSUS,
        HAS_REQUEST,
    >
where
    Ctx: Context,
{
    /// Use a custom WAL actor reference.
    #[must_use]
    pub fn with_custom_wal(
        self,
        wal_ref: WalRef<Ctx>,
    ) -> EngineBuilder<
        Ctx,
        Config,
        Signer,
        NoCodec,
        NetCodec,
        SyncCodec,
        true,
        HAS_NETWORK,
        HAS_SYNC,
        HAS_CONSENSUS,
        HAS_REQUEST,
    > {
        EngineBuilder {
            ctx: self.ctx,
            config: self.config,
            wal: Some(WalBuilder::Custom(wal_ref)),
            network: self.network,
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }
}

// Implementation for custom Network actor
impl<
        Ctx,
        Config,
        Signer,
        WalCodec,
        SyncCodec,
        const HAS_WAL: bool,
        const HAS_SYNC: bool,
        const HAS_CONSENSUS: bool,
        const HAS_REQUEST: bool,
    >
    EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NoCodec,
        SyncCodec,
        HAS_WAL,
        false,
        HAS_SYNC,
        HAS_CONSENSUS,
        HAS_REQUEST,
    >
where
    Ctx: Context,
{
    /// Use a custom Network actor reference and message sender.
    #[must_use]
    pub fn with_custom_network(
        self,
        network_ref: NetworkRef<Ctx>,
        tx_network: Sender<NetworkMsg<Ctx>>,
    ) -> EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NoCodec,
        SyncCodec,
        HAS_WAL,
        true,
        HAS_SYNC,
        HAS_CONSENSUS,
        HAS_REQUEST,
    > {
        EngineBuilder {
            ctx: self.ctx,
            config: self.config,
            wal: self.wal,
            network: Some(NetworkBuilder::Custom((network_ref, tx_network))),
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }
}

// Implementation for custom Sync actor
impl<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        const HAS_WAL: bool,
        const HAS_NETWORK: bool,
        const HAS_CONSENSUS: bool,
        const HAS_REQUEST: bool,
    >
    EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        NoCodec,
        HAS_WAL,
        HAS_NETWORK,
        false,
        HAS_CONSENSUS,
        HAS_REQUEST,
    >
where
    Ctx: Context,
{
    /// Use a custom Sync actor reference.
    #[must_use]
    pub fn with_custom_sync(
        self,
        sync_ref: SyncRef<Ctx>,
    ) -> EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        NoCodec,
        HAS_WAL,
        HAS_NETWORK,
        true,
        HAS_CONSENSUS,
        HAS_REQUEST,
    > {
        EngineBuilder {
            ctx: self.ctx,
            config: self.config,
            wal: self.wal,
            network: self.network,
            sync: Some(SyncBuilder::Custom(Some(sync_ref))),
            consensus: self.consensus,
            request: self.request,
        }
    }

    /// Disable the Sync actor.
    #[must_use]
    pub fn with_no_sync(
        self,
    ) -> EngineBuilder<
        Ctx,
        Config,
        Signer,
        WalCodec,
        NetCodec,
        NoCodec,
        HAS_WAL,
        HAS_NETWORK,
        true,
        HAS_CONSENSUS,
        HAS_REQUEST,
    > {
        EngineBuilder {
            ctx: self.ctx,
            config: self.config,
            wal: self.wal,
            network: self.network,
            sync: Some(SyncBuilder::Custom(None)),
            consensus: self.consensus,
            request: self.request,
        }
    }
}

// Implementation for build() - only available when ALL actors are configured
impl<Ctx, Config, Signer, WalCodec, NetCodec, SyncCodec>
    EngineBuilder<Ctx, Config, Signer, WalCodec, NetCodec, SyncCodec, true, true, true, true, true>
where
    Ctx: Context,
    Config: NodeConfig,
    Signer: SigningProvider<Ctx> + 'static,
    WalCodec: codec::WalCodec<Ctx>,
    NetCodec: codec::ConsensusCodec<Ctx> + codec::SyncCodec<Ctx>,
    SyncCodec: codec::SyncCodec<Ctx>,
{
    /// Build and start the engine with the configured actors.
    ///
    /// This method is only available when all required actors have been configured:
    /// - WAL (via `with_wal_builder`)
    /// - Network (via `with_network_builder`)
    /// - Consensus (via `with_consensus_builder`)
    /// - Sync (via `with_sync_builder`)
    /// - Request (via `with_request_builder`)
    ///
    /// The build process will:
    /// 1. Spawn actors in dependency order (network → wal → host → consensus → sync → node)
    /// 2. Set up request handling tasks
    /// 3. Return channels for the application and the engine handle
    pub async fn build(self) -> Result<(Channels<Ctx>, EngineHandle)> {
        // SAFETY: All these unwrap() calls are safe because the const generic
        // constraints guarantee that all configurations are present.
        let RequestBuilder::Default(request_ctx) = self.request.unwrap();
        let ConsensusBuilder::Default(consensus_ctx) = self.consensus.unwrap();
        let wal_builder = self.wal.unwrap();
        let network_builder = self.network.unwrap();
        let sync_builder = self.sync.unwrap();

        // Set up metrics
        let registry = SharedRegistry::global().with_moniker(self.config.moniker());
        let metrics = Metrics::register(&registry);

        // 1. Network actor (default or custom)
        let (network, tx_network) = match network_builder {
            NetworkBuilder::Custom(custom) => custom,
            NetworkBuilder::Default(network_ctx) => {
                spawn_network_actor(
                    network_ctx.identity,
                    self.config.consensus(),
                    self.config.value_sync(),
                    &registry,
                    network_ctx.codec,
                )
                .await?
            }
        };

        // 2. WAL actor (default or custom)
        let wal = match wal_builder {
            WalBuilder::Custom(wal_ref) => wal_ref,
            WalBuilder::Default(wal_ctx) => {
                spawn_wal_actor(&self.ctx, wal_ctx.codec, &wal_ctx.path, &registry).await?
            }
        };

        // 3. Host actor (use the default channel-based Connector)
        let (connector, rx_consensus) = spawn_host_actor(metrics.clone()).await?;

        let tx_event = TxEvent::new();
        let sync_port = Arc::new(OutputPort::new());

        // 4. Consensus actor (spawned before sync so sync can reference it)
        let consensus = spawn_consensus_actor(
            self.ctx.clone(),
            consensus_ctx.address,
            self.config.consensus().clone(),
            Box::new(consensus_ctx.signing_provider),
            network.clone(),
            connector.clone(),
            wal.clone(),
            sync_port.clone(),
            metrics,
            tx_event.clone(),
        )
        .await?;

        // 5. Sync actor (default or custom)
        let sync = match sync_builder {
            SyncBuilder::Custom(sync_ref) => sync_ref,
            SyncBuilder::Default(sync_ctx) => {
                spawn_sync_actor(
                    self.ctx.clone(),
                    network.clone(),
                    connector.clone(),
                    consensus.clone(),
                    sync_ctx.codec,
                    self.config.value_sync(),
                    &registry,
                )
                .await?
            }
        };

        // Subscribe sync actor to the sync port
        if let Some(sync) = &sync {
            sync.subscribe_to_port(&sync_port);
        }

        // 6. Node actor
        let (node, handle) = spawn_node_actor(
            self.ctx,
            network.clone(),
            consensus.clone(),
            wal,
            sync,
            connector,
        )
        .await?;

        // Spawn request handling tasks
        let (tx_request, rx_request) = mpsc::channel(request_ctx.channel_size);
        crate::run::spawn_consensus_request_task(rx_request, consensus);

        let (tx_net_request, rx_net_request) = mpsc::channel(request_ctx.channel_size);
        crate::run::spawn_network_request_task(rx_net_request, network);

        // Build channels and handle
        let channels = Channels {
            consensus: rx_consensus,
            network: tx_network,
            events: tx_event,
            requests: tx_request,
            net_requests: tx_net_request,
        };

        let handle = EngineHandle::new(node, handle);

        Ok((channels, handle))
    }
}

#[cfg(test)]
mod tests {
    use malachitebft_test::codec::json::JsonCodec;
    use malachitebft_test::codec::proto::ProtobufCodec;
    use malachitebft_test::{Ed25519Provider, TestContext};

    use super::*;

    fn fake<A>() -> A {
        unreachable!()
    }

    struct Config;

    impl NodeConfig for Config {
        fn moniker(&self) -> &str {
            "test-node"
        }

        fn consensus(&self) -> &malachitebft_config::ConsensusConfig {
            todo!()
        }

        fn consensus_mut(&mut self) -> &mut malachitebft_config::ConsensusConfig {
            todo!()
        }

        fn value_sync(&self) -> &malachitebft_config::ValueSyncConfig {
            todo!()
        }

        fn value_sync_mut(&mut self) -> &mut malachitebft_config::ValueSyncConfig {
            todo!()
        }
    }

    // All default actors
    #[allow(dead_code)]
    async fn all_defaults_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_default_wal(WalContext::new(fake(), ProtobufCodec))
            .with_default_network(NetworkContext::new(fake(), JsonCodec))
            .with_default_sync(SyncContext::new(JsonCodec))
            .with_default_consensus(ConsensusContext::new(fake(), fake::<Ed25519Provider>()))
            .with_default_request(RequestContext::new(100))
            .build()
            .await;
    }

    // All custom actors
    #[allow(dead_code)]
    async fn all_custom_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_custom_wal(fake())
            .with_custom_network(fake(), fake())
            .with_custom_sync(fake())
            .with_default_consensus(ConsensusContext::new(fake(), fake::<Ed25519Provider>()))
            .with_default_request(RequestContext::new(100))
            .build()
            .await;
    }

    // Custom WAL, default everything else
    #[allow(dead_code)]
    async fn custom_wal_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_custom_wal(fake())
            .with_default_network(NetworkContext::new(fake(), JsonCodec))
            .with_default_sync(SyncContext::new(JsonCodec))
            .with_default_consensus(ConsensusContext::new(fake(), fake::<Ed25519Provider>()))
            .with_default_request(RequestContext::new(100))
            .build()
            .await;
    }

    // Custom network, default everything else
    #[allow(dead_code)]
    async fn custom_network_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_default_wal(WalContext::new(fake(), ProtobufCodec))
            .with_custom_network(fake(), fake())
            .with_default_sync(SyncContext::new(JsonCodec))
            .with_default_consensus(ConsensusContext::new(fake(), fake::<Ed25519Provider>()))
            .with_default_request(RequestContext::new(100))
            .build()
            .await;
    }

    // Custom sync, default everything else
    #[allow(dead_code)]
    async fn custom_sync_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_default_wal(WalContext::new(fake(), ProtobufCodec))
            .with_default_network(NetworkContext::new(fake(), JsonCodec))
            .with_custom_sync(fake())
            .with_default_consensus(ConsensusContext::new(fake(), fake::<Ed25519Provider>()))
            .with_default_request(RequestContext::new(100))
            .build()
            .await;
    }

    // Disabled sync (with_no_sync)
    #[allow(dead_code)]
    async fn no_sync_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_default_wal(WalContext::new(fake(), ProtobufCodec))
            .with_default_network(NetworkContext::new(fake(), JsonCodec))
            .with_no_sync()
            .with_default_consensus(ConsensusContext::new(fake(), fake::<Ed25519Provider>()))
            .with_default_request(RequestContext::new(100))
            .build()
            .await;
    }

    // Mixed: custom WAL and network, default sync
    #[allow(dead_code)]
    async fn custom_wal_and_network_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_custom_wal(fake())
            .with_custom_network(fake(), fake())
            .with_default_sync(SyncContext::new(JsonCodec))
            .with_default_consensus(ConsensusContext::new(fake(), fake::<Ed25519Provider>()))
            .with_default_request(RequestContext::new(100))
            .build()
            .await;
    }

    // Mixed: default WAL, custom network and sync
    #[allow(dead_code)]
    async fn custom_network_and_sync_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_default_wal(WalContext::new(fake(), ProtobufCodec))
            .with_custom_network(fake(), fake())
            .with_custom_sync(fake())
            .with_default_consensus(ConsensusContext::new(fake(), fake::<Ed25519Provider>()))
            .with_default_request(RequestContext::new(100))
            .build()
            .await;
    }

    // Different order of configuration (should still work)
    #[allow(dead_code)]
    async fn different_order_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_default_consensus(ConsensusContext::new(fake(), fake::<Ed25519Provider>()))
            .with_default_request(RequestContext::new(100))
            .with_default_wal(WalContext::new(fake(), ProtobufCodec))
            .with_default_network(NetworkContext::new(fake(), JsonCodec))
            .with_default_sync(SyncContext::new(JsonCodec))
            .build()
            .await;
    }
}
