//! Run Malachite consensus with the given configuration and context.
//! Provides the application with a channel for receiving messages from consensus.

use std::path::PathBuf;

use eyre::Result;

use crate::app::types::codec::{ConsensusCodec, SyncCodec, WalCodec};
use crate::app::types::config::Config as NodeConfig;
use crate::app::types::core::Context;
use crate::app::types::metrics::{Metrics, SharedRegistry};
use crate::spawn::{spawn_host_actor, spawn_network_actor};
use crate::{app, Channels};

use malachitebft_app::{spawn_consensus_actor, spawn_sync_actor, spawn_wal_actor};
use malachitebft_engine::util::events::TxEvent;

#[tracing::instrument("node", skip_all, fields(moniker = %cfg.moniker))]
pub async fn run<Node, Ctx, Codec>(
    ctx: Ctx,
    codec: Codec,
    node: Node,
    cfg: NodeConfig,
    private_key_file: PathBuf,
    start_height: Option<Ctx::Height>,
    initial_validator_set: Ctx::ValidatorSet,
) -> Result<Channels<Ctx>>
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

    let private_key_file = node.load_private_key_file(private_key_file)?;
    let private_key = node.load_private_key(private_key_file);
    let public_key = node.get_public_key(&private_key);
    let address = node.get_address(&public_key);
    let keypair = node.get_keypair(private_key.clone());
    let signing_provider = node.get_signing_provider(private_key);

    // Spawn consensus gossip
    let (network, network_tx) =
        spawn_network_actor(&cfg, keypair, &registry, codec.clone()).await?;

    let wal = spawn_wal_actor(&ctx, codec, &node.get_home_dir(), &registry).await?;

    // Spawn the host actor
    let (connector, consensus_rx) = spawn_host_actor(metrics.clone()).await?;

    let sync = spawn_sync_actor(
        ctx.clone(),
        network.clone(),
        connector.clone(),
        &cfg.sync,
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
        Box::new(signing_provider),
        network,
        connector,
        wal,
        sync.clone(),
        metrics,
        TxEvent::new(),
    )
    .await?;

    Ok(Channels {
        consensus: consensus_rx,
        network: network_tx,
    })
}
