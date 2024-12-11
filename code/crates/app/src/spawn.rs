//! Utility functions for spawning the actor system and connecting it to the application.

use std::path::Path;
use std::time::Duration;

use eyre::Result;
use tracing::Span;

use malachite_actors::block_sync::{
    BlockSync, BlockSyncCodec, BlockSyncRef, Params as BlockSyncParams,
};
use malachite_actors::consensus::{Consensus, ConsensusCodec, ConsensusParams, ConsensusRef};
use malachite_actors::gossip_consensus::{GossipConsensus, GossipConsensusRef};
use malachite_actors::host::HostRef;
use malachite_actors::util::events::TxEvent;
use malachite_actors::wal::{Wal, WalCodec, WalRef};
use malachite_gossip_consensus::{
    Config as GossipConsensusConfig, DiscoveryConfig, GossipSubConfig, Keypair,
};

use crate::types::config::{
    BlockSyncConfig, Config as NodeConfig, PubSubProtocol, TransportProtocol,
};
use crate::types::core::Context;
use crate::types::metrics::{Metrics, SharedRegistry};
use crate::types::sync;
use crate::types::ValuePayload;

pub async fn spawn_gossip_consensus_actor<Ctx, Codec>(
    cfg: &NodeConfig,
    keypair: Keypair,
    registry: &SharedRegistry,
    codec: Codec,
) -> Result<GossipConsensusRef<Ctx>>
where
    Ctx: Context,
    Codec: ConsensusCodec<Ctx>,
    Codec: BlockSyncCodec<Ctx>,
{
    let config = make_gossip_config(cfg);

    GossipConsensus::spawn(keypair, config, registry.clone(), codec, Span::current())
        .await
        .map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
pub async fn spawn_consensus_actor<Ctx>(
    start_height: Ctx::Height,
    initial_validator_set: Ctx::ValidatorSet,
    address: Ctx::Address,
    ctx: Ctx,
    cfg: NodeConfig,
    gossip_consensus: GossipConsensusRef<Ctx>,
    host: HostRef<Ctx>,
    wal: WalRef<Ctx>,
    block_sync: Option<BlockSyncRef<Ctx>>,
    metrics: Metrics,
    tx_event: TxEvent<Ctx>,
) -> Result<ConsensusRef<Ctx>>
where
    Ctx: Context,
{
    use crate::types::config;
    let value_payload = match cfg.consensus.value_payload {
        config::ValuePayload::PartsOnly => ValuePayload::PartsOnly,
        config::ValuePayload::ProposalOnly => ValuePayload::ProposalOnly,
        config::ValuePayload::ProposalAndParts => ValuePayload::ProposalAndParts,
    };

    let consensus_params = ConsensusParams {
        start_height,
        initial_validator_set,
        address,
        threshold_params: Default::default(),
        value_payload,
    };

    Consensus::spawn(
        ctx,
        consensus_params,
        cfg.consensus.timeouts,
        gossip_consensus,
        host,
        wal,
        block_sync,
        metrics,
        tx_event,
        Span::current(),
    )
    .await
    .map_err(Into::into)
}

pub async fn spawn_wal_actor<Ctx, Codec>(
    ctx: &Ctx,
    codec: Codec,
    home_dir: &Path,
    registry: &SharedRegistry,
) -> Result<WalRef<Ctx>>
where
    Ctx: Context,
    Codec: WalCodec<Ctx>,
{
    let wal_dir = home_dir.join("wal");
    std::fs::create_dir_all(&wal_dir).unwrap();

    let wal_file = wal_dir.join("consensus.wal");

    Wal::spawn(ctx, codec, wal_file, registry.clone(), Span::current())
        .await
        .map_err(Into::into)
}

pub async fn spawn_block_sync_actor<Ctx>(
    ctx: Ctx,
    gossip_consensus: GossipConsensusRef<Ctx>,
    host: HostRef<Ctx>,
    config: &BlockSyncConfig,
    initial_height: Ctx::Height,
    registry: &SharedRegistry,
) -> Result<Option<BlockSyncRef<Ctx>>>
where
    Ctx: Context,
{
    if !config.enabled {
        return Ok(None);
    }

    let params = BlockSyncParams {
        status_update_interval: config.status_update_interval,
        request_timeout: config.request_timeout,
    };

    let metrics = sync::Metrics::register(registry);
    let block_sync = BlockSync::new(
        ctx,
        gossip_consensus,
        host,
        params,
        metrics,
        Span::current(),
    );

    let actor_ref = block_sync.spawn(initial_height).await?;
    Ok(Some(actor_ref))
}

fn make_gossip_config(cfg: &NodeConfig) -> GossipConsensusConfig {
    GossipConsensusConfig {
        listen_addr: cfg.consensus.p2p.listen_addr.clone(),
        persistent_peers: cfg.consensus.p2p.persistent_peers.clone(),
        discovery: DiscoveryConfig {
            enabled: cfg.consensus.p2p.discovery.enabled,
            ..Default::default()
        },
        idle_connection_timeout: Duration::from_secs(15 * 60),
        transport: match cfg.consensus.p2p.transport {
            TransportProtocol::Tcp => malachite_gossip_consensus::TransportProtocol::Tcp,
            TransportProtocol::Quic => malachite_gossip_consensus::TransportProtocol::Quic,
        },
        pubsub_protocol: match cfg.consensus.p2p.protocol {
            PubSubProtocol::GossipSub(_) => malachite_gossip_consensus::PubSubProtocol::GossipSub,
            PubSubProtocol::Broadcast => malachite_gossip_consensus::PubSubProtocol::Broadcast,
        },
        gossipsub: match cfg.consensus.p2p.protocol {
            PubSubProtocol::GossipSub(config) => GossipSubConfig {
                mesh_n: config.mesh_n(),
                mesh_n_high: config.mesh_n_high(),
                mesh_n_low: config.mesh_n_low(),
                mesh_outbound_min: config.mesh_outbound_min(),
            },
            PubSubProtocol::Broadcast => GossipSubConfig::default(),
        },
        rpc_max_size: cfg.consensus.p2p.rpc_max_size.as_u64() as usize,
        pubsub_max_size: cfg.consensus.p2p.pubsub_max_size.as_u64() as usize,
    }
}
