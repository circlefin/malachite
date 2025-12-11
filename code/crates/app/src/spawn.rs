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
use malachitebft_network::{
    ChannelNames, Config as NetworkConfig, DiscoveryConfig, GossipSubConfig, Keypair,
};
use malachitebft_signing::SigningProvider;
use malachitebft_sync as sync;

use crate::config::{ConsensusConfig, ValueSyncConfig};
use crate::metrics::{Metrics, SharedRegistry};
use crate::types::core::Context;
use crate::types::ValuePayload;

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
    consensus_cfg: &ConsensusConfig,
    value_sync_cfg: &ValueSyncConfig,
    moniker: String,
    keypair: Keypair,
    registry: &SharedRegistry,
    codec: Codec,
) -> Result<NetworkRef<Ctx>>
where
    Ctx: Context,
    Codec: ConsensusCodec<Ctx>,
    Codec: SyncCodec<Ctx>,
{
    let config = make_network_config(consensus_cfg, value_sync_cfg, moniker);

    Network::spawn(keypair, config, registry.clone(), codec, Span::current())
        .await
        .map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
pub async fn spawn_consensus_actor<Ctx>(
    ctx: Ctx,
    address: Ctx::Address,
    mut cfg: ConsensusConfig,
    sync_cfg: &ValueSyncConfig,
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

    let consensus_params = ConsensusParams {
        address,
        threshold_params: Default::default(),
        value_payload,
        enabled: cfg.enabled,
    };

    // Derive the consensus queue capacity from `sync.parallel_requests` and `sync.batch_size`
    cfg.queue_capacity = sync_cfg.parallel_requests * sync_cfg.batch_size;

    Consensus::spawn(
        ctx,
        consensus_params,
        cfg,
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
    path: &Path,
    registry: &SharedRegistry,
) -> Result<WalRef<Ctx>>
where
    Ctx: Context,
    Codec: WalCodec<Ctx>,
{
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }

    Wal::spawn(
        ctx,
        codec,
        path.to_owned(),
        registry.clone(),
        Span::current(),
    )
    .await
    .map_err(Into::into)
}

pub async fn spawn_sync_actor<Ctx, Codec>(
    ctx: Ctx,
    network: NetworkRef<Ctx>,
    host: HostRef<Ctx>,
    sync_codec: Codec,
    config: &ValueSyncConfig,
    registry: &SharedRegistry,
) -> Result<Option<SyncRef<Ctx>>>
where
    Ctx: Context,
    Codec: SyncCodec<Ctx>,
{
    if !config.enabled {
        return Ok(None);
    }

    let params = SyncParams {
        status_update_interval: config.status_update_interval,
        request_timeout: config.request_timeout,
    };

    let scoring_strategy = match config.scoring_strategy {
        malachitebft_config::ScoringStrategy::Ema => sync::scoring::Strategy::Ema,
    };

    let sync_config = sync::Config {
        enabled: config.enabled,
        max_request_size: config.max_request_size.as_u64() as usize,
        max_response_size: config.max_response_size.as_u64() as usize,
        request_timeout: config.request_timeout,
        parallel_requests: config.parallel_requests as u64,
        scoring_strategy,
        inactive_threshold: (!config.inactive_threshold.is_zero())
            .then_some(config.inactive_threshold),
        batch_size: config.batch_size,
    };

    let metrics = sync::Metrics::register(registry);

    let actor_ref = Sync::spawn(
        ctx,
        network,
        host,
        params,
        sync_codec,
        sync_config,
        metrics,
        Span::current(),
    )
    .await?;

    Ok(Some(actor_ref))
}

fn make_network_config(
    cfg: &ConsensusConfig,
    value_sync_cfg: &ValueSyncConfig,
    moniker: String,
) -> NetworkConfig {
    use malachitebft_config as config;
    use malachitebft_network as network;

    NetworkConfig {
        moniker,
        listen_addr: cfg.p2p.listen_addr.clone(),
        persistent_peers: cfg.p2p.persistent_peers.clone(),
        persistent_peers_only: cfg.p2p.persistent_peers_only,
        discovery: DiscoveryConfig {
            enabled: cfg.p2p.discovery.enabled,
            persistent_peers_only: cfg.p2p.persistent_peers_only,
            bootstrap_protocol: match cfg.p2p.discovery.bootstrap_protocol {
                config::BootstrapProtocol::Kademlia => network::BootstrapProtocol::Kademlia,
                config::BootstrapProtocol::Full => network::BootstrapProtocol::Full,
            },
            selector: match cfg.p2p.discovery.selector {
                config::Selector::Kademlia => network::Selector::Kademlia,
                config::Selector::Random => network::Selector::Random,
            },
            num_outbound_peers: cfg.p2p.discovery.num_outbound_peers,
            num_inbound_peers: cfg.p2p.discovery.num_inbound_peers,
            max_connections_per_peer: cfg.p2p.discovery.max_connections_per_peer,
            ephemeral_connection_timeout: cfg.p2p.discovery.ephemeral_connection_timeout,
            dial_max_retries: cfg.p2p.discovery.dial_max_retries,
            request_max_retries: cfg.p2p.discovery.request_max_retries,
            connect_request_max_retries: cfg.p2p.discovery.connect_request_max_retries,
        },
        idle_connection_timeout: Duration::from_secs(15 * 60),
        transport: network::TransportProtocol::from_multiaddr(&cfg.p2p.listen_addr).unwrap_or_else(
            || {
                panic!(
                    "No valid transport protocol found in listen address: {}",
                    cfg.p2p.listen_addr
                )
            },
        ),
        pubsub_protocol: match cfg.p2p.protocol {
            config::PubSubProtocol::GossipSub(_) => network::PubSubProtocol::GossipSub,
            config::PubSubProtocol::Broadcast => network::PubSubProtocol::Broadcast,
        },
        gossipsub: match cfg.p2p.protocol {
            config::PubSubProtocol::GossipSub(config) => GossipSubConfig {
                mesh_n: config.mesh_n(),
                mesh_n_high: config.mesh_n_high(),
                mesh_n_low: config.mesh_n_low(),
                mesh_outbound_min: config.mesh_outbound_min(),
                enable_peer_scoring: config.enable_peer_scoring(),
            },
            config::PubSubProtocol::Broadcast => GossipSubConfig::default(),
        },
        channel_names: ChannelNames::default(),
        rpc_max_size: cfg.p2p.rpc_max_size.as_u64() as usize,
        pubsub_max_size: cfg.p2p.pubsub_max_size.as_u64() as usize,
        enable_consensus: cfg.enabled,
        enable_sync: value_sync_cfg.enabled,
        protocol_names: network::ProtocolNames {
            consensus: cfg.p2p.protocol_names.consensus.clone(),
            discovery_kad: cfg.p2p.protocol_names.discovery_kad.clone(),
            discovery_regres: cfg.p2p.protocol_names.discovery_regres.clone(),
            sync: cfg.p2p.protocol_names.sync.clone(),
        },
    }
}
