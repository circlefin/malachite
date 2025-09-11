//! Run Malachite consensus with the given configuration and context.
//! Provides the application with a channel for receiving messages from consensus.

use eyre::Result;

use crate::app::metrics::{Metrics, SharedRegistry};
use crate::app::node::{self, EngineHandle, NodeConfig};
use crate::app::spawn::{
    spawn_consensus_actor, spawn_node_actor, spawn_sync_actor, spawn_wal_actor,
};
use crate::app::types::codec;
use crate::app::types::core::Context;
use crate::msgs::ConsensusRequest;
use crate::spawn::{spawn_host_actor, spawn_network_actor};
use crate::Channels;
use malachitebft_app::types::sync;
use malachitebft_engine::consensus::{ConsensusMsg, ConsensusRef};
use malachitebft_engine::util::events::TxEvent;
use tokio::sync::mpsc::Receiver;

pub async fn start_engine<Node, Ctx, WalCodec, NetCodec>(
    ctx: Ctx,
    node: Node,
    cfg: Node::Config,
    wal_codec: WalCodec,
    net_codec: NetCodec,
    start_height: Option<Ctx::Height>,
    initial_validator_set: Ctx::ValidatorSet,
) -> Result<(Channels<Ctx>, EngineHandle)>
where
    Ctx: Context,
    Node: node::Node<Context = Ctx>,
    WalCodec: codec::WalCodec<Ctx> + Clone,
    NetCodec: codec::ConsensusCodec<Ctx>,
    NetCodec: codec::SyncCodec<Ctx>,
    NetCodec: codec::HasEncodedLen<sync::Response<Ctx>>,
{
    let start_height = start_height.unwrap_or_default();

    let registry = SharedRegistry::global().with_moniker(cfg.moniker());
    let metrics = Metrics::register(&registry);

    let private_key_file = node.load_private_key_file()?;
    let private_key = node.load_private_key(private_key_file);
    let public_key = node.get_public_key(&private_key);
    let address = node.get_address(&public_key);
    let keypair = node.get_keypair(private_key.clone());
    let signing_provider = node.get_signing_provider(private_key);

    // Spawn consensus gossip
    let (network, tx_network) =
        spawn_network_actor(cfg.consensus(), keypair, &registry, net_codec).await?;

    let wal = spawn_wal_actor(&ctx, wal_codec, &node.get_home_dir(), &registry).await?;

    // Spawn the host actor
    let (connector, rx_consensus) = spawn_host_actor(metrics.clone()).await?;

    let sync = spawn_sync_actor(
        ctx.clone(),
        network.clone(),
        connector.clone(),
        cfg.value_sync(),
        &registry,
    )
    .await?;

    let tx_event = TxEvent::new();

    // Spawn consensus
    let consensus = spawn_consensus_actor(
        start_height,
        initial_validator_set,
        address,
        ctx.clone(),
        cfg.consensus().clone(),
        cfg.value_sync(),
        Box::new(signing_provider),
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

    let (tx_request, rx_request) = tokio::sync::mpsc::channel(100);
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
