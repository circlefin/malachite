//! Run Malachite consensus with the given configuration and context.
//! Provides the application with a channel for receiving messages from consensus.

use eyre::Result;

use malachitebft_engine::util::events::TxEvent;

use crate::app;
use crate::app::spawn::{
    spawn_consensus_actor, spawn_node_actor, spawn_sync_actor, spawn_wal_actor,
};
use crate::app::types::codec::{ConsensusCodec, SyncCodec, WalCodec};
use crate::app::types::config::Config as NodeConfig;
use crate::app::types::core::Context;
use crate::app::types::metrics::{Metrics, SharedRegistry};
use crate::app::EngineHandle;
use crate::spawn::{spawn_host_actor, spawn_network_actor};
use crate::Channels;

#[tracing::instrument("node", skip_all, fields(moniker = %cfg.moniker))]
pub async fn start_engine<Node, Ctx, Codec>(
    ctx: Ctx,
    codec: Codec,
    node: Node,
    cfg: NodeConfig,
    start_height: Option<Ctx::Height>,
    initial_validator_set: Ctx::ValidatorSet,
) -> Result<(Channels<Ctx>, EngineHandle)>
where
    Ctx: Context,
    Node: app::Node<Context = Ctx>,
    Codec: WalCodec<Ctx> + Clone,
    Codec: ConsensusCodec<Ctx>,
    Codec: SyncCodec<Ctx>,
{
    let start_height = start_height.unwrap_or_default();

    let registry = SharedRegistry::global().with_moniker(cfg.moniker.as_str());
    let metrics = Metrics::register(&registry);

    let private_key_file = node.load_private_key_file()?;
    let private_key = node.load_private_key(private_key_file);
    let public_key = node.get_public_key(&private_key);
    let address = node.get_address(&public_key);
    let keypair = node.get_keypair(private_key.clone());
    let signing_provider = node.get_signing_provider(private_key);

    // Spawn consensus gossip
    let (network, tx_network) =
        spawn_network_actor(&cfg, keypair, &registry, codec.clone()).await?;

    let wal = spawn_wal_actor(&ctx, codec, &node.get_home_dir(), &registry).await?;

    // Spawn the host actor
    let (connector, rx_consensus) = spawn_host_actor(metrics.clone()).await?;

    let sync = spawn_sync_actor(
        ctx.clone(),
        network.clone(),
        connector.clone(),
        &cfg.value_sync,
        &cfg.consensus.vote_sync,
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
        cfg,
        Box::new(signing_provider),
        network.clone(),
        connector.clone(),
        wal.clone(),
        sync.clone(),
        metrics,
        tx_event.clone(),
    )
    .await?;

    let (node, handle) = spawn_node_actor(ctx, network, consensus, wal, sync, connector).await?;

    let channels = Channels {
        consensus: rx_consensus,
        network: tx_network,
        events: tx_event,
    };

    let handle = EngineHandle {
        actor: node,
        handle,
    };

    Ok((channels, handle))
}
