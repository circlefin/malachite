//! Run Malachite consensus with the given configuration and context.
//! Provides the application with a channel for receiving messages from consensus.

use eyre::Result;
use tokio::sync::mpsc;

use crate::app;
use crate::app::types::codec::{BlockSyncCodec, ConsensusCodec, WalCodec};
use crate::app::types::config::Config as NodeConfig;
use crate::app::types::core::Context;
use crate::app::types::metrics::{Metrics, SharedRegistry};
use crate::channel::AppMsg;
use crate::spawn::spawn_host_actor;

use malachite_actors::util::events::TxEvent;
use malachite_app::{
    spawn_block_sync_actor, spawn_consensus_actor, spawn_gossip_consensus_actor, spawn_wal_actor,
};

#[tracing::instrument("node", skip_all, fields(moniker = %cfg.moniker))]
pub async fn run<Node, Ctx, Codec>(
    cfg: NodeConfig,
    start_height: Option<Ctx::Height>,
    ctx: Ctx,
    codec: Codec,
    node: Node,
    initial_validator_set: Ctx::ValidatorSet,
) -> Result<mpsc::Receiver<AppMsg<Ctx>>>
where
    Ctx: Context,
    Node: app::Node<Context = Ctx>,
    Codec: WalCodec<Ctx> + Clone,
    Codec: ConsensusCodec<Ctx>,
    Codec: BlockSyncCodec<Ctx>,
{
    let start_height = start_height.unwrap_or_default();

    let registry = SharedRegistry::global().with_moniker(cfg.moniker.as_str());
    let metrics = Metrics::register(&registry);

    // TODO: Simplify this?
    let private_key_file = node.load_private_key_file(node.get_home_dir())?;
    let private_key = node.load_private_key(private_key_file);
    let public_key = node.get_public_key(&private_key);
    let address = node.get_address(&public_key);
    let keypair = node.get_keypair(private_key);

    // Spawn consensus gossip
    let gossip_consensus =
        spawn_gossip_consensus_actor(&cfg, keypair, &registry, codec.clone()).await?;

    let wal = spawn_wal_actor(&ctx, codec, &node.get_home_dir(), &registry).await?;

    // Spawn the host actor
    let (connector, rx) = spawn_host_actor(metrics.clone()).await?;

    let block_sync = spawn_block_sync_actor(
        ctx.clone(),
        gossip_consensus.clone(),
        connector.clone(),
        &cfg.blocksync,
        start_height,
        &registry,
    )
    .await?;

    // Spawn consensus
    let _consensus = spawn_consensus_actor(
        start_height,
        initial_validator_set,
        address,
        ctx,
        cfg,
        gossip_consensus,
        connector,
        wal,
        block_sync.clone(),
        metrics,
        TxEvent::new(),
    )
    .await?;

    Ok(rx)
}
