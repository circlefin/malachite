//! Run Malachite consensus with the given configuration and context.
//! Provides the application with a channel for receiving messages from consensus.

use std::path::PathBuf;

use eyre::{eyre, Result};
use tokio::sync::mpsc::{self, Receiver};
use tokio::task::JoinHandle;

use malachitebft_app::types::Keypair;
use malachitebft_engine::consensus::{ConsensusMsg, ConsensusRef};
use malachitebft_engine::node::NodeRef;
use malachitebft_engine::util::events::TxEvent;
use malachitebft_signing::SigningProvider;

use crate::app::config::NodeConfig;
use crate::app::metrics::{Metrics, SharedRegistry};
use crate::app::spawn::{
    spawn_consensus_actor, spawn_node_actor, spawn_sync_actor, spawn_wal_actor,
};
use crate::app::types::codec;
use crate::app::types::core::Context;
use crate::msgs::ConsensusRequest;
use crate::spawn::{spawn_host_actor, spawn_network_actor};
use crate::Channels;

pub struct EngineHandle {
    pub actor: NodeRef,
    pub handle: JoinHandle<()>,
}

impl EngineHandle {
    pub fn new(actor: NodeRef, handle: JoinHandle<()>) -> Self {
        Self { actor, handle }
    }
}

pub struct NetworkContext<Codec> {
    pub keypair: Keypair,
    pub codec: Codec,
}

impl<Codec> NetworkContext<Codec> {
    pub fn new(keypair: Keypair, codec: Codec) -> Self {
        Self { keypair, codec }
    }
}

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

pub struct WalContext<Codec> {
    pub path: PathBuf,
    pub codec: Codec,
}

impl<Codec> WalContext<Codec> {
    pub fn new(path: PathBuf, codec: Codec) -> Self {
        Self { path, codec }
    }
}

pub struct RequestContext {
    pub channel_size: usize,
}

impl RequestContext {
    pub fn new(channel_size: usize) -> Self {
        Self { channel_size }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn start_engine<Ctx, Config, Signer, WalCodec, NetCodec>(
    ctx: Ctx,
    cfg: Config,
    wal_ctx: WalContext<WalCodec>,
    network_ctx: NetworkContext<NetCodec>,
    consensus_ctx: ConsensusContext<Ctx, Signer>,
    request_ctx: RequestContext,
) -> Result<(Channels<Ctx>, EngineHandle)>
where
    Ctx: Context,
    Config: NodeConfig,
    Signer: SigningProvider<Ctx> + 'static,
    WalCodec: codec::WalCodec<Ctx> + Clone,
    NetCodec: Clone,
    NetCodec: codec::ConsensusCodec<Ctx>,
    NetCodec: codec::SyncCodec<Ctx>,
{
    let registry = SharedRegistry::global().with_moniker(cfg.moniker());
    let metrics = Metrics::register(&registry);

    // Spawn consensus gossip
    let (network, tx_network) = spawn_network_actor(
        cfg.consensus(),
        cfg.value_sync(),
        cfg.moniker().to_string(),
        network_ctx.keypair,
        &registry,
        network_ctx.codec.clone(),
    )
    .await?;

    let wal = spawn_wal_actor(&ctx, wal_ctx.codec, &wal_ctx.path, &registry).await?;

    // Spawn the host actor
    let (connector, rx_consensus) = spawn_host_actor(metrics.clone()).await?;

    if cfg.value_sync().enabled && cfg.value_sync().batch_size == 0 {
        return Err(eyre!("Value sync batch size cannot be zero"));
    }

    let sync = spawn_sync_actor(
        ctx.clone(),
        network.clone(),
        connector.clone(),
        network_ctx.codec,
        cfg.value_sync(),
        &registry,
    )
    .await?;

    let tx_event = TxEvent::new();

    // Spawn consensus
    let consensus = spawn_consensus_actor(
        ctx.clone(),
        consensus_ctx.address,
        cfg.consensus().clone(),
        cfg.value_sync(),
        Box::new(consensus_ctx.signing_provider),
        network.clone(),
        connector.clone(),
        wal.clone(),
        sync.clone(),
        metrics,
        tx_event.clone(),
    )
    .await?;

    let (node, handle) =
        spawn_node_actor(ctx, network, consensus.clone(), wal, sync, connector).await?;

    let (tx_request, rx_request) = mpsc::channel(request_ctx.channel_size);
    spawn_request_task(rx_request, consensus);

    let channels = Channels {
        consensus: rx_consensus,
        network: tx_network,
        events: tx_event,
        requests: tx_request,
    };

    let handle = EngineHandle {
        actor: node,
        handle,
    };

    Ok((channels, handle))
}

fn spawn_request_task<Ctx>(
    mut rx_request: Receiver<ConsensusRequest<Ctx>>,
    consensus: ConsensusRef<Ctx>,
) where
    Ctx: Context,
{
    tokio::spawn(async move {
        while let Some(msg) = rx_request.recv().await {
            match msg {
                ConsensusRequest::DumpState(reply) => {
                    if let Err(e) = consensus.cast(ConsensusMsg::DumpState(reply.into())) {
                        tracing::error!("Failed to send state dump request: {e}");
                    }
                }
            }
        }
    });
}
