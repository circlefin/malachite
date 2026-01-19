//! Builder pattern for constructing the consensus engine with optional custom actors.
//!
//! This module provides a type-safe builder that uses const generics to track
//! at compile-time which actors have been configured. The `build()` method is
//! only available when all required actors have been configured.

use std::path::PathBuf;

use eyre::Result;
use tokio::sync::mpsc::{self, Sender};

use malachitebft_engine::network::{NetworkIdentity, NetworkRef};
use malachitebft_engine::sync::SyncRef;
use malachitebft_engine::util::events::TxEvent;
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
enum WalBuilder<Ctx: Context, Codec> {
    Default(WalContext<Codec>),
    Custom(WalRef<Ctx>),
}

/// Builder for the Network actor - either default or custom.
enum NetworkBuilder<Ctx: Context, Codec> {
    Default(NetworkContext<Codec>),
    Custom((NetworkRef<Ctx>, Sender<NetworkMsg<Ctx>>)),
}

/// Builder for the Sync actor - either default or custom.
/// The inner Option allows explicitly disabling sync via `with_sync_actor(None)`.
enum SyncBuilder<Ctx: Context, Codec> {
    Default(SyncContext<Codec>),
    Custom(Option<SyncRef<Ctx>>),
}

/// Builder for the Consensus actor.
enum ConsensusBuilder<Ctx: Context, Signer> {
    Default(ConsensusContext<Ctx, Signer>),
}

/// Builder for request channels.
enum RequestBuilder {
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
/// let (channels, handle) = EngineBuilder::new(ctx, config)
///     .with_default_wal(WalContext::new(path, codec))
///     .with_default_network(NetworkContext::new(identity, codec))
///     .with_default_sync(SyncContext::new(sync_codec))
///     .consensus_context(ConsensusContext::new(address, signer))
///     .request_context(RequestContext::new(100))
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
///     .with_default_sync(SyncContext::new(sync_codec))
///     .consensus_context(ConsensusContext::new(address, signer))
///     .request_context(RequestContext::new(100))
///     .with_network_actor(network_ref, tx_network)
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
    const HAS_WAL: bool,
    const HAS_NETWORK: bool,
    const HAS_SYNC: bool,
    const HAS_CONSENSUS: bool,
    const HAS_REQUEST: bool,
> where
    Ctx: Context,
    Config: NodeConfig,
    Signer: SigningProvider<Ctx> + 'static,
    WalCodec: codec::WalCodec<Ctx>,
    NetCodec: codec::ConsensusCodec<Ctx> + codec::SyncCodec<Ctx>,
    SyncCodec: codec::SyncCodec<Ctx>,
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
    EngineBuilder<Ctx, Config, Signer, WalCodec, NetCodec, SyncCodec, false, false, false, false, false>
where
    Ctx: Context,
    Config: NodeConfig,
    Signer: SigningProvider<Ctx>,
    WalCodec: codec::WalCodec<Ctx>,
    NetCodec: codec::ConsensusCodec<Ctx> + codec::SyncCodec<Ctx>,
    SyncCodec: codec::SyncCodec<Ctx>,
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
    Config: NodeConfig,
    Signer: SigningProvider<Ctx>,
    WalCodec: codec::WalCodec<Ctx>,
    NetCodec: codec::ConsensusCodec<Ctx> + codec::SyncCodec<Ctx>,
    SyncCodec: codec::SyncCodec<Ctx>,
{
    /// Set the consensus context (required).
    ///
    /// This configures the consensus actor with the node's address and signing provider.
    pub fn consensus_context(
        self,
        ctx: ConsensusContext<Ctx, Signer>,
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
            consensus: Some(ConsensusBuilder::Default(ctx)),
            request: self.request,
        }
    }

    /// Set the request context (required).
    ///
    /// This configures the channel size for consensus and network request channels.
    pub fn request_context(
        self,
        ctx: RequestContext,
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
            request: Some(RequestBuilder::Default(ctx)),
        }
    }

    /// Use the default WAL actor.
    ///
    /// Required unless providing a custom WAL actor via `with_wal_actor`.
    pub fn with_default_wal(
        self,
        ctx: WalContext<WalCodec>,
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
            wal: Some(WalBuilder::Default(ctx)),
            network: self.network,
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }

    /// Use the default Network actor.
    ///
    /// Required unless providing a custom Network actor via `with_network_actor`.
    pub fn with_default_network(
        self,
        ctx: NetworkContext<NetCodec>,
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
            network: Some(NetworkBuilder::Default(ctx)),
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }

    /// Use the default Sync actor.
    ///
    /// Required unless providing a custom Sync actor via `with_sync_actor`.
    pub fn with_default_sync(
        self,
        ctx: SyncContext<SyncCodec>,
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
            sync: Some(SyncBuilder::Default(ctx)),
            consensus: self.consensus,
            request: self.request,
        }
    }

    /// Provide a custom network actor instead of spawning the default one.
    ///
    /// ## Arguments
    /// - `network_ref`: The actor reference passed to other actors
    /// - `tx_network`: Channel for the application to send network messages
    pub fn with_network_actor(
        self,
        network_ref: NetworkRef<Ctx>,
        tx_network: Sender<NetworkMsg<Ctx>>,
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
            network: Some(NetworkBuilder::Custom((network_ref, tx_network))),
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }

    /// Provide a custom WAL actor instead of spawning the default one.
    pub fn with_wal_actor(
        self,
        wal_ref: WalRef<Ctx>,
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
            wal: Some(WalBuilder::Custom(wal_ref)),
            network: self.network,
            sync: self.sync,
            consensus: self.consensus,
            request: self.request,
        }
    }

    /// Provide a custom sync actor instead of spawning the default one.
    ///
    /// Pass `None` to explicitly disable sync, or `Some(sync_ref)` to use a custom sync actor.
    pub fn with_sync_actor(
        self,
        sync_ref: Option<SyncRef<Ctx>>,
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
            sync: Some(SyncBuilder::Custom(sync_ref)),
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
    Signer: SigningProvider<Ctx>,
    WalCodec: codec::WalCodec<Ctx>,
    NetCodec: codec::ConsensusCodec<Ctx> + codec::SyncCodec<Ctx>,
    SyncCodec: codec::SyncCodec<Ctx>,
{
    /// Build and start the engine with the configured actors.
    ///
    /// This method is only available when all required actors have been configured:
    /// - WAL (via `with_default_wal` or `with_wal_actor`)
    /// - Network (via `with_default_network` or `with_network_actor`)
    /// - Sync (via `with_default_sync` or `with_sync_actor`)
    /// - Consensus (via `consensus_context`)
    /// - Request (via `request_context`)
    ///
    /// The build process will:
    /// 1. Spawn actors in dependency order (network → wal → host → sync → consensus → node)
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

        // 4. Sync actor (default or custom)
        let sync = match sync_builder {
            SyncBuilder::Custom(sync_ref) => sync_ref,
            SyncBuilder::Default(sync_ctx) => {
                spawn_sync_actor(
                    self.ctx.clone(),
                    network.clone(),
                    connector.clone(),
                    sync_ctx.codec,
                    self.config.value_sync(),
                    &registry,
                )
                .await?
            }
        };

        let tx_event = TxEvent::new();

        // 5. Consensus actor
        let consensus = spawn_consensus_actor(
            self.ctx.clone(),
            consensus_ctx.address,
            self.config.consensus().clone(),
            self.config.value_sync(),
            Box::new(consensus_ctx.signing_provider),
            network.clone(),
            connector.clone(),
            wal.clone(),
            sync.clone(),
            metrics,
            tx_event.clone(),
        )
        .await?;

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
