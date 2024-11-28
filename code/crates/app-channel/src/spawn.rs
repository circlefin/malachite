use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::mpsc;

use malachite_actors::block_sync::{BlockSync, BlockSyncRef, Params as BlockSyncParams};
use malachite_actors::consensus::{Consensus, ConsensusParams, ConsensusRef};
use malachite_actors::gossip_consensus::{GossipConsensus, GossipConsensusRef};
use malachite_actors::util::codec::NetworkCodec;
use malachite_actors::util::streaming::StreamMessage;
use malachite_common::{CommitCertificate, Context};
use malachite_config::{BlockSyncConfig, Config as NodeConfig, PubSubProtocol, TransportProtocol};
use malachite_consensus::{SignedConsensusMsg, ValuePayload};
use malachite_gossip_consensus::{
    Config as GossipConsensusConfig, DiscoveryConfig, GossipSubConfig, Keypair,
};
use malachite_metrics::{Metrics, SharedRegistry};

use crate::channel::AppMsg;
use crate::connector::Connector;

pub async fn spawn_gossip_consensus_actor<Ctx, Codec>(
    cfg: &NodeConfig,
    keypair: Keypair,
    registry: &SharedRegistry,
    codec: Codec,
) -> GossipConsensusRef<Ctx>
where
    Ctx: Context,
    Codec: NetworkCodec<Ctx::ProposalPart>,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
    Codec: NetworkCodec<StreamMessage<Ctx::ProposalPart>>,
    Codec: NetworkCodec<malachite_blocksync::Status<Ctx>>,
    Codec: NetworkCodec<malachite_blocksync::Request<Ctx>>,
    Codec: NetworkCodec<malachite_blocksync::Response<Ctx>>,
{
    let config_gossip = GossipConsensusConfig {
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
    };

    GossipConsensus::spawn(keypair, config_gossip, registry.clone(), codec)
        .await
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
pub async fn spawn_consensus_actor<Ctx>(
    start_height: Ctx::Height,
    initial_validator_set: Ctx::ValidatorSet,
    address: Ctx::Address,
    ctx: Ctx,
    cfg: NodeConfig,
    gossip_consensus: GossipConsensusRef<Ctx>,
    host: malachite_actors::host::HostRef<Ctx>,
    block_sync: Option<BlockSyncRef<Ctx>>,
    metrics: Metrics,
    tx_decision: Option<broadcast::Sender<CommitCertificate<Ctx>>>,
) -> ConsensusRef<Ctx>
where
    Ctx: Context,
{
    let value_payload = match cfg.consensus.value_payload {
        malachite_config::ValuePayload::PartsOnly => ValuePayload::PartsOnly,
        malachite_config::ValuePayload::ProposalOnly => ValuePayload::ProposalOnly,
        malachite_config::ValuePayload::ProposalAndParts => ValuePayload::ProposalAndParts,
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
        block_sync,
        metrics,
        tx_decision,
    )
    .await
    .unwrap()
}

pub async fn spawn_block_sync_actor<Ctx>(
    ctx: Ctx,
    gossip_consensus: GossipConsensusRef<Ctx>,
    host: malachite_actors::host::HostRef<Ctx>,
    config: &BlockSyncConfig,
    initial_height: Ctx::Height,
    registry: &SharedRegistry,
) -> Option<BlockSyncRef<Ctx>>
where
    Ctx: Context,
{
    if !config.enabled {
        return None;
    }

    let params = BlockSyncParams {
        status_update_interval: config.status_update_interval,
        request_timeout: config.request_timeout,
    };

    let metrics = malachite_blocksync::Metrics::register(registry);
    let block_sync = BlockSync::new(ctx, gossip_consensus, host, params, metrics);
    let (actor_ref, _) = block_sync.spawn(initial_height).await.unwrap();

    Some(actor_ref)
}

#[allow(clippy::too_many_arguments)]
pub async fn spawn_host_actor<Ctx>(
    metrics: Metrics,
) -> (
    malachite_actors::host::HostRef<Ctx>,
    mpsc::Receiver<AppMsg<Ctx>>,
)
where
    Ctx: Context,
{
    let (tx, rx) = mpsc::channel(1);

    let actor_ref = Connector::spawn(tx, metrics).await.unwrap();

    (actor_ref, rx)
}
