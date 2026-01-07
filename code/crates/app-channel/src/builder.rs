//! Builder pattern for constructing the consensus engine with optional custom actors.

use std::path::PathBuf;

use eyre::{eyre, Result};
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

/// Builder for constructing the consensus engine with optional custom actors.
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
///     .consensus_context(ConsensusContext::new(address, signer))
///     .request_context(RequestContext::new(100))
///     .with_network_actor(network_ref, tx_network)
///     .build()
///     .await?;
/// ```
pub struct EngineBuilder<Ctx, Config, Signer, WalCodec, NetCodec, SyncCodec>
where
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

    // Context structs (required unless using custom actors)
    wal_ctx: Option<WalContext<WalCodec>>,
    network_ctx: Option<NetworkContext<NetCodec>>,
    consensus_ctx: Option<ConsensusContext<Ctx, Signer>>,
    sync_ctx: Option<SyncContext<SyncCodec>>,
    request_ctx: Option<RequestContext>,

    // Optional custom actors
    custom_network: Option<(NetworkRef<Ctx>, Sender<NetworkMsg<Ctx>>)>,
    custom_wal: Option<WalRef<Ctx>>,
    custom_sync: Option<Option<SyncRef<Ctx>>>,
}

impl<Ctx, Config, Signer, WalCodec, NetCodec, SyncCodec>
    EngineBuilder<Ctx, Config, Signer, WalCodec, NetCodec, SyncCodec>
where
    Ctx: Context,
    Config: NodeConfig,
    Signer: SigningProvider<Ctx>,
    WalCodec: codec::WalCodec<Ctx>,
    NetCodec: codec::ConsensusCodec<Ctx> + codec::SyncCodec<Ctx>,
    SyncCodec: codec::SyncCodec<Ctx>,
{
    /// Create a new engine builder with the required context and configuration.
    pub fn new(ctx: Ctx, config: Config) -> Self {
        Self {
            ctx,
            config,
            wal_ctx: None,
            network_ctx: None,
            consensus_ctx: None,
            sync_ctx: None,
            request_ctx: None,
            custom_network: None,
            custom_wal: None,
            custom_sync: None,
        }
    }

    /// Set the consensus context (required).
    pub fn consensus_context(mut self, ctx: ConsensusContext<Ctx, Signer>) -> Self {
        self.consensus_ctx = Some(ctx);
        self
    }

    /// Set the request context (required).
    pub fn request_context(mut self, ctx: RequestContext) -> Self {
        self.request_ctx = Some(ctx);
        self
    }

    /// Use the default WAL actor.
    ///
    /// Required unless providing a custom WAL actor.
    pub fn with_default_wal(mut self, ctx: WalContext<WalCodec>) -> Self {
        self.wal_ctx = Some(ctx);
        self
    }

    /// Use the default Network actor.
    ///
    /// Required unless providing a custom Network actor.
    pub fn with_default_network(mut self, ctx: NetworkContext<NetCodec>) -> Self {
        self.network_ctx = Some(ctx);
        self
    }

    /// Use the default Sync actor.
    ///
    /// Required unless providing a custom Sync actor or disabling sync.
    pub fn with_default_sync(mut self, ctx: SyncContext<SyncCodec>) -> Self {
        self.sync_ctx = Some(ctx);
        self
    }

    /// Provide a custom network actor instead of spawning the default one.
    ///
    /// ## Arguments
    /// - `NetworkRef<Ctx>`: The actor reference passed to other actors
    /// - `Sender<NetworkMsg<Ctx>>`: Channel for the application to send network messages
    pub fn with_network_actor(
        mut self,
        network_ref: NetworkRef<Ctx>,
        tx_network: Sender<NetworkMsg<Ctx>>,
    ) -> Self {
        self.custom_network = Some((network_ref, tx_network));
        self
    }

    /// Provide a custom WAL actor instead of spawning the default one.
    pub fn with_wal_actor(mut self, wal_ref: WalRef<Ctx>) -> Self {
        self.custom_wal = Some(wal_ref);
        self
    }

    /// Provide a custom sync actor instead of spawning the default one.
    ///
    /// Note: The sync actor is already optional based on configuration.
    /// Pass `None` to explicitly disable sync, or `Some(sync_ref)` to use a custom sync actor.
    /// If neither is provided, the default sync actor will be spawned.
    pub fn with_sync_actor(mut self, sync_ref: Option<SyncRef<Ctx>>) -> Self {
        self.custom_sync = Some(sync_ref);
        self
    }

    /// Build and start the engine with the configured actors.
    ///
    /// This method will:
    /// 1. Validate that all required contexts are provided
    /// 2. Spawn default actors for any that weren't custom-provided
    /// 3. Respect dependency order (network → wal → host → sync → consensus → node)
    /// 4. Set up request handling tasks
    /// 5. Return channels for the application and the engine handle
    pub async fn build(self) -> Result<(Channels<Ctx>, EngineHandle)> {
        // Request context is always required
        let request_ctx = self
            .request_ctx
            .ok_or_else(|| eyre!("Request context is required"))?;

        // Set up metrics
        let registry = SharedRegistry::global().with_moniker(self.config.moniker());
        let metrics = Metrics::register(&registry);

        // 1. Network actor (or use custom)
        let (network, tx_network) = if let Some(custom) = self.custom_network {
            custom
        } else {
            let network_ctx = self.network_ctx.ok_or_else(|| {
                eyre!("Network context is required unless using custom network actor")
            })?;

            spawn_network_actor(
                network_ctx.identity,
                self.config.consensus(),
                self.config.value_sync(),
                &registry,
                network_ctx.codec,
            )
            .await?
        };

        // 2. WAL actor (or use custom)
        let wal = if let Some(custom) = self.custom_wal {
            custom
        } else {
            let wal_ctx = self
                .wal_ctx
                .ok_or_else(|| eyre!("WAL context is required unless using custom WAL actor"))?;

            spawn_wal_actor(&self.ctx, wal_ctx.codec, &wal_ctx.path, &registry).await?
        };

        // 3. Host actor (use the default channel-based Connector)
        let (connector, rx_consensus) = spawn_host_actor(metrics.clone()).await?;

        // 4. Sync actor (or use custom, or skip if disabled)
        let sync = if let Some(custom) = self.custom_sync {
            custom
        } else {
            if self.config.value_sync().enabled && self.config.value_sync().batch_size == 0 {
                return Err(eyre!("Value sync batch size cannot be zero"));
            }

            let sync_ctx = self.sync_ctx.ok_or_else(|| {
                eyre!("Sync context is required for spawning sync actor (or provide custom sync actor)")
            })?;

            spawn_sync_actor(
                self.ctx.clone(),
                network.clone(),
                connector.clone(),
                sync_ctx.codec,
                self.config.value_sync(),
                &registry,
            )
            .await?
        };

        let tx_event = TxEvent::new();

        // 5. Consensus actor (or use custom)
        let consensus = {
            let consensus_ctx = self.consensus_ctx.ok_or_else(|| {
                eyre!("Consensus context is required unless using custom consensus actor")
            })?;

            spawn_consensus_actor(
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
            .await?
        };

        // 6. Node actor (or use custom)
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
