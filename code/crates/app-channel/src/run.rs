//! Run Malachite consensus with the given configuration and context.
//! Provides the application with a channel for receiving messages from consensus.

use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

use eyre::Result;

use malachitebft_engine::consensus::{ConsensusMsg, ConsensusRef};
use malachitebft_engine::network::{NetworkMsg, NetworkRef};
use malachitebft_engine::node::NodeRef;
use malachitebft_signing::SigningProvider;

pub use malachitebft_engine::network::NetworkIdentity;
pub use malachitebft_signing::SigningProviderExt;

// Re-export context structs from builder module
pub use crate::builder::{
    ConsensusContext, NetworkContext, RequestContext, SyncContext, WalContext,
};

use crate::app::config::NodeConfig;
use crate::app::types::codec;
use crate::app::types::core::Context;
use crate::msgs::{ConsensusRequest, NetworkRequest};
use crate::{Channels, EngineBuilder};

pub struct EngineHandle {
    pub actor: NodeRef,
    pub handle: JoinHandle<()>,
}

impl EngineHandle {
    pub fn new(actor: NodeRef, handle: JoinHandle<()>) -> Self {
        Self { actor, handle }
    }
}

/// Start the consensus engine with default actors.
///
/// This is a convenience function that uses [`EngineBuilder`](crate::EngineBuilder) internally.
/// For more control over actor spawning (e.g., providing custom actor implementations),
/// use [`EngineBuilder`](crate::EngineBuilder) directly.
///
/// # Example
/// ```rust,ignore
/// // Sign the validator proof (ADR-006) and build the network identity
/// let proof = signer.sign_validator_proof(public_key_bytes, peer_id_bytes).await?;
/// let proof_bytes = net_codec.encode(&proof)?;
/// let identity = NetworkIdentity::new_validator(moniker, keypair, address, proof_bytes);
///
/// let (channels, handle) = start_engine(
///     ctx,
///     config,
///     WalContext::new(path, wal_codec),
///     NetworkContext::new(identity, net_codec),
///     ConsensusContext::new(address, signer),
///     SyncContext::new(sync_codec),
///     RequestContext::new(100),
/// ).await?;
/// ```
pub async fn start_engine<Ctx, Config, Signer, WalCodec, NetCodec, SyncCodec>(
    ctx: Ctx,
    cfg: Config,
    wal_ctx: WalContext<WalCodec>,
    network_ctx: NetworkContext<NetCodec>,
    consensus_ctx: ConsensusContext<Ctx, Signer>,
    sync_ctx: SyncContext<SyncCodec>,
    request_ctx: RequestContext,
) -> Result<(Channels<Ctx>, EngineHandle)>
where
    Ctx: Context,
    Config: NodeConfig,
    Signer: SigningProvider<Ctx> + 'static,
    WalCodec: codec::WalCodec<Ctx> + Clone,
    NetCodec: Clone + codec::ConsensusCodec<Ctx> + codec::SyncCodec<Ctx>,
    SyncCodec: Clone + codec::SyncCodec<Ctx>,
{
    EngineBuilder::new(ctx, cfg)
        .with_default_wal(wal_ctx)
        .with_default_network(network_ctx)
        .with_default_sync(sync_ctx)
        .with_default_consensus(consensus_ctx)
        .with_default_request(request_ctx)
        .build()
        .await
}

pub(crate) fn spawn_consensus_request_task<Ctx>(
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

pub(crate) fn spawn_network_request_task<Ctx>(
    mut rx_request: Receiver<NetworkRequest>,
    network: NetworkRef<Ctx>,
) where
    Ctx: Context,
{
    tokio::spawn(async move {
        while let Some(msg) = rx_request.recv().await {
            match msg {
                NetworkRequest::DumpState(reply) => {
                    if let Err(error) = network.cast(NetworkMsg::DumpState(reply.into())) {
                        tracing::error!(%error, "Failed to send network state dump request");
                    }
                }
                NetworkRequest::UpdatePersistentPeers(op, reply) => {
                    if let Err(error) =
                        network.cast(NetworkMsg::UpdatePersistentPeers(op, reply.into()))
                    {
                        tracing::error!(%error, "Failed to send update persistent peers request");
                    }
                }
            }
        }
    });
}
