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

impl<Ctx: Context, Codec> WalBuilder<Ctx, Codec> {
    /// Use the default WAL actor with the given context.
    pub fn default(context: WalContext<Codec>) -> Self {
        Self::Default(context)
    }
}

impl<Ctx: Context> WalBuilder<Ctx, NoCodec> {
    /// Use a custom WAL actor reference.
    pub fn custom(wal_ref: WalRef<Ctx>) -> Self {
        Self::Custom(wal_ref)
    }
}

/// Builder for the Network actor - either default or custom.
#[allow(clippy::large_enum_variant)]
pub enum NetworkBuilder<Ctx: Context, Codec> {
    /// Use the default Network actor with the given context.
    Default(NetworkContext<Codec>),
    /// Use a custom Network actor reference and message sender.
    Custom((NetworkRef<Ctx>, Sender<NetworkMsg<Ctx>>)),
}

impl<Ctx: Context, Codec> NetworkBuilder<Ctx, Codec> {
    /// Use the default Network actor with the given context.
    pub fn default(context: NetworkContext<Codec>) -> Self {
        Self::Default(context)
    }
}

impl<Ctx: Context> NetworkBuilder<Ctx, NoCodec> {
    /// Use a custom Network actor reference and message sender.
    pub fn custom(network_ref: NetworkRef<Ctx>, tx_network: Sender<NetworkMsg<Ctx>>) -> Self {
        Self::Custom((network_ref, tx_network))
    }
}

/// Builder for the Sync actor - either default, custom, or disabled.
pub enum SyncBuilder<Ctx: Context, Codec> {
    /// Use the default Sync actor with the given context.
    Default(SyncContext<Codec>),
    /// Use a custom Sync actor reference, or `None` to disable sync.
    Custom(Option<SyncRef<Ctx>>),
}

impl<Ctx: Context, Codec> SyncBuilder<Ctx, Codec> {
    /// Use the default Sync actor with the given context.
    pub fn default(context: SyncContext<Codec>) -> Self {
        Self::Default(context)
    }
}

impl<Ctx: Context> SyncBuilder<Ctx, NoCodec> {
    /// Use a custom Sync actor reference, or `None` to disable sync.
    pub fn custom(sync_ref: Option<SyncRef<Ctx>>) -> Self {
        Self::Custom(sync_ref)
    }
}

/// Builder for the Consensus actor.
pub enum ConsensusBuilder<Ctx: Context, Signer> {
    /// Use the default Consensus actor with the given context.
    Default(ConsensusContext<Ctx, Signer>),
}

impl<Ctx: Context, Signer> ConsensusBuilder<Ctx, Signer> {
    /// Use the default Consensus actor with the given context.
    pub fn default(context: ConsensusContext<Ctx, Signer>) -> Self {
        Self::Default(context)
    }
}

/// Builder for request channels.
pub enum RequestBuilder {
    /// Use the default request channel configuration.
    Default(RequestContext),
}

impl RequestBuilder {
    /// Use the default request channel configuration.
    pub fn default(context: RequestContext) -> Self {
        Self::Default(context)
    }
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
/// let (channels, handle) = EngineBuilder::new(ctx, config)
///     .with_wal_builder(WalBuilder::Default(WalContext::new(path, codec)))
///     .with_network_builder(NetworkBuilder::Default(NetworkContext::new(identity, codec)))
///     .with_sync_builder(SyncBuilder::Default(SyncContext::new(sync_codec)))
///     .with_consensus_builder(ConsensusBuilder::Default(ConsensusContext::new(address, signer)))
///     .with_request_builder(RequestBuilder::Default(RequestContext::new(100)))
///     .build()
///     .await?;
/// ```
///
/// # Example: Custom network actor
/// ```rust,ignore
/// let (network_ref, tx_network) = spawn_custom_network_actor().await?;
///
/// let (channels, handle) = EngineBuilder::new(ctx, config)
///     .with_wal_builder(WalBuilder::Default(WalContext::new(path, codec)))
///     .with_network_builder(NetworkBuilder::Custom((network_ref, tx_network)))
///     .with_sync_builder(SyncBuilder::Default(SyncContext::new(sync_codec)))
///     .with_consensus_builder(ConsensusBuilder::Default(ConsensusContext::new(address, signer)))
///     .with_request_builder(RequestBuilder::Default(RequestContext::new(100)))
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

// Implementation for creating a new builder (all flags start as false)
impl<Ctx, Config, Signer, WalCodec, NetCodec, SyncCodec>
    EngineBuilder<Ctx, Config, Signer, WalCodec, NetCodec, SyncCodec>
where
    Ctx: Context,
{
    /// Create a new engine builder with the required context and configuration.
    ///
    /// All actor configurations start unconfigured. You must configure all required
    /// actors (WAL, Network, Sync, Consensus, Request) before `build()` becomes available.
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
    /// Set the WAL builder.
    ///
    /// Use `WalBuilder::Default(WalContext::new(...))` for the default WAL actor,
    /// or `WalBuilder::Custom(wal_ref)` for a custom implementation.
    #[must_use]
    pub fn with_wal_builder(
        self,
        builder: WalBuilder<Ctx, WalCodec>,
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
            wal: Some(builder),
            network: self.network,
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }

    /// Set the Network builder.
    ///
    /// Use `NetworkBuilder::Default(NetworkContext::new(...))` for the default Network actor,
    /// or `NetworkBuilder::Custom((network_ref, tx_network))` for a custom implementation.
    #[must_use]
    pub fn with_network_builder(
        self,
        builder: NetworkBuilder<Ctx, NetCodec>,
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
            network: Some(builder),
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }

    /// Set the Sync builder.
    ///
    /// Use `SyncBuilder::Default(SyncContext::new(...))` for the default Sync actor,
    /// or `SyncBuilder::Custom(Some(sync_ref))` for a custom implementation,
    /// or `SyncBuilder::Custom(None)` to disable sync.
    #[must_use]
    pub fn with_sync_builder(
        self,
        builder: SyncBuilder<Ctx, SyncCodec>,
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
            sync: Some(builder),
            consensus: self.consensus,
            request: self.request,
        }
    }

    /// Set the Consensus builder.
    ///
    /// Use `ConsensusBuilder::Default(ConsensusContext::new(...))` for the default Consensus actor.
    #[must_use]
    pub fn with_consensus_builder(
        self,
        builder: ConsensusBuilder<Ctx, Signer>,
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
            consensus: Some(builder),
            request: self.request,
        }
    }

    /// Set the Request builder.
    ///
    /// Use `RequestBuilder::Default(RequestContext::new(...))` for the default request channels.
    #[must_use]
    pub fn with_request_builder(
        self,
        builder: RequestBuilder,
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
            request: Some(builder),
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
            self.config.value_sync(),
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

    #[allow(dead_code)]
    async fn custom_builder_compiles() {
        let ctx = TestContext::default();

        let _ = EngineBuilder::new(ctx, Config)
            .with_wal_builder(WalBuilder::custom(fake()))
            .with_network_builder(NetworkBuilder::custom(fake(), fake()))
            .with_sync_builder(SyncBuilder::default(SyncContext::new(JsonCodec)))
            .with_consensus_builder(ConsensusBuilder::default(ConsensusContext::new(
                fake(),
                fake::<Ed25519Provider>(),
            )))
            .with_request_builder(RequestBuilder::Default(RequestContext::new(100)))
            .build()
            .await;
    }
}
