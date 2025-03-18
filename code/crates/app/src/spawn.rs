//! Utility functions for spawning the actor system and connecting it to the application.

use std::path::Path;
use std::time::Duration;

use eyre::Result;
use tokio::task::JoinHandle;
use tracing::Span;

use malachitebft_engine::consensus::{Consensus, ConsensusCodec, ConsensusParams, ConsensusRef};
use malachitebft_engine::host::HostRef;
use malachitebft_engine::network::{Network, NetworkRef};
use malachitebft_engine::node::{Node, NodeRef};
use malachitebft_engine::sync::{Params as SyncParams, Sync, SyncCodec, SyncRef};
use malachitebft_engine::util::events::TxEvent;
use malachitebft_engine::wal::{Wal, WalCodec, WalRef};
use malachitebft_network::{Config as NetworkConfig, DiscoveryConfig, GossipSubConfig, Keypair};

use crate::config::{
    self, ConsensusConfig, PubSubProtocol, TransportProtocol, ValueSyncConfig, VoteSyncConfig,
};
use crate::metrics::{Metrics, SharedRegistry};
use crate::types::core::{Context, SigningProvider};
use crate::types::sync;
use crate::types::{ValuePayload, VoteSyncMode};

pub async fn spawn_node_actor<Ctx>(
    ctx: Ctx,
    network: NetworkRef<Ctx>,
    consensus: ConsensusRef<Ctx>,
    wal: WalRef<Ctx>,
    sync: Option<SyncRef<Ctx>>,
    host: HostRef<Ctx>,
) -> Result<(NodeRef, JoinHandle<()>)>
where
    Ctx: Context,
{
    // Spawn the node actor
    let node = Node::new(
        ctx,
        network,
        consensus,
        wal,
        sync,
        host,
        tracing::Span::current(),
    );

    let (actor_ref, handle) = node.spawn().await?;
    Ok((actor_ref, handle))
}

pub async fn spawn_network_actor<Ctx, Codec>(
    cfg: &ConsensusConfig,
    keypair: Keypair,
    registry: &SharedRegistry,
    codec: Codec,
) -> Result<NetworkRef<Ctx>>
where
    Ctx: Context,
    Codec: ConsensusCodec<Ctx>,
    Codec: SyncCodec<Ctx>,
{
    let config = make_gossip_config(cfg);

    Network::spawn(keypair, config, registry.clone(), codec, Span::current())
        .await
        .map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
pub async fn spawn_consensus_actor<Ctx>(
    initial_height: Ctx::Height,
    initial_validator_set: Ctx::ValidatorSet,
    address: Ctx::Address,
    ctx: Ctx,
    cfg: &ConsensusConfig,
    signing_provider: Box<dyn SigningProvider<Ctx>>,
    network: NetworkRef<Ctx>,
    host: HostRef<Ctx>,
    wal: WalRef<Ctx>,
    sync: Option<SyncRef<Ctx>>,
    metrics: Metrics,
    tx_event: TxEvent<Ctx>,
) -> Result<ConsensusRef<Ctx>>
where
    Ctx: Context,
{
    use crate::config;

    let value_payload = match cfg.value_payload {
        config::ValuePayload::PartsOnly => ValuePayload::PartsOnly,
        config::ValuePayload::ProposalOnly => ValuePayload::ProposalOnly,
        config::ValuePayload::ProposalAndParts => ValuePayload::ProposalAndParts,
    };

    let vote_sync_mode = match cfg.vote_sync.mode {
        config::VoteSyncMode::RequestResponse => VoteSyncMode::RequestResponse,
        config::VoteSyncMode::Rebroadcast => VoteSyncMode::Rebroadcast,
    };

    let consensus_params = ConsensusParams {
        initial_height,
        initial_validator_set,
        address,
        threshold_params: Default::default(),
        value_payload,
        vote_sync_mode,
    };

    Consensus::spawn(
        ctx,
        consensus_params,
        cfg.timeouts,
        signing_provider,
        network,
        host,
        wal,
        sync,
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

pub async fn spawn_sync_actor<Ctx>(
    ctx: Ctx,
    network: NetworkRef<Ctx>,
    host: HostRef<Ctx>,
    config: &ValueSyncConfig,
    vote_sync: &VoteSyncConfig,
    registry: &SharedRegistry,
) -> Result<Option<SyncRef<Ctx>>>
where
    Ctx: Context,
{
    if !config.enabled && vote_sync.mode != config::VoteSyncMode::RequestResponse {
        return Ok(None);
    }

    let params = SyncParams {
        status_update_interval: config.status_update_interval,
        request_timeout: config.request_timeout,
    };

    let metrics = sync::Metrics::register(registry);

    let actor_ref = Sync::spawn(ctx, network, host, params, metrics, Span::current()).await?;

    Ok(Some(actor_ref))
}

fn make_gossip_config(cfg: &ConsensusConfig) -> NetworkConfig {
    NetworkConfig {
        listen_addr: cfg.p2p.listen_addr.clone(),
        persistent_peers: cfg.p2p.persistent_peers.clone(),
        discovery: DiscoveryConfig {
            enabled: cfg.p2p.discovery.enabled,
            ..Default::default()
        },
        idle_connection_timeout: Duration::from_secs(15 * 60),
        transport: match cfg.p2p.transport {
            TransportProtocol::Tcp => malachitebft_network::TransportProtocol::Tcp,
            TransportProtocol::Quic => malachitebft_network::TransportProtocol::Quic,
        },
        pubsub_protocol: match cfg.p2p.protocol {
            PubSubProtocol::GossipSub(_) => malachitebft_network::PubSubProtocol::GossipSub,
            PubSubProtocol::Broadcast => malachitebft_network::PubSubProtocol::Broadcast,
        },
        gossipsub: match cfg.p2p.protocol {
            PubSubProtocol::GossipSub(config) => GossipSubConfig {
                mesh_n: config.mesh_n(),
                mesh_n_high: config.mesh_n_high(),
                mesh_n_low: config.mesh_n_low(),
                mesh_outbound_min: config.mesh_outbound_min(),
            },
            PubSubProtocol::Broadcast => GossipSubConfig::default(),
        },
        rpc_max_size: cfg.p2p.rpc_max_size.as_u64() as usize,
        pubsub_max_size: cfg.p2p.pubsub_max_size.as_u64() as usize,
        enable_sync: cfg.vote_sync.mode.is_request_response(),
    }
}
