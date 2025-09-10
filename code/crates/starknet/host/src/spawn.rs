use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::warn;

use malachitebft_config::{self as config, MempoolConfig, MempoolLoadConfig, ValueSyncConfig};
use malachitebft_core_types::ValuePayload;
use malachitebft_engine::consensus::{Consensus, ConsensusParams, ConsensusRef};
use malachitebft_engine::host::HostRef;
use malachitebft_engine::network::{Network, NetworkRef};
use malachitebft_engine::node::{Node, NodeRef};
use malachitebft_engine::sync::{Params as SyncParams, Sync, SyncRef};
use malachitebft_engine::util::events::TxEvent;
use malachitebft_engine::wal::{Wal, WalRef};
use malachitebft_metrics::{Metrics as ConsensusMetrics, SharedRegistry};
use malachitebft_network::{ChannelNames, Keypair};
use malachitebft_starknet_p2p_types::Ed25519Provider;
use malachitebft_sync as sync;
use malachitebft_test_mempool::Config as MempoolNetworkConfig;

use crate::actor::Host;
use crate::codec::ProtobufCodec;
use crate::config::Config;
use crate::host::{StarknetHost, StarknetParams};
use crate::mempool::network::{MempoolNetwork, MempoolNetworkRef};
use crate::mempool::{Mempool, MempoolRef};
use crate::mempool_load::{MempoolLoad, MempoolLoadRef, Params};
use crate::metrics::Metrics as AppMetrics;
use crate::types::MockContext;
use crate::types::{Address, Height, PrivateKey, ValidatorSet};

pub async fn spawn_node_actor(
    cfg: Config,
    home_dir: PathBuf,
    initial_validator_set: ValidatorSet,
    private_key: PrivateKey,
    start_height: Option<Height>,
    tx_event: TxEvent<MockContext>,
    span: tracing::Span,
) -> (NodeRef, JoinHandle<()>) {
    let ctx = MockContext::new();

    let start_height = start_height.unwrap_or(Height::new(1, 1));

    let registry = SharedRegistry::global().with_moniker(cfg.moniker.as_str());
    let consensus_metrics = ConsensusMetrics::register(&registry);
    let app_metrics = AppMetrics::register(&registry);
    let sync_metrics = sync::Metrics::register(&registry);

    let address = Address::from_public_key(private_key.public_key());
    let signing_provider = Ed25519Provider::new(private_key.clone());

    // Spawn mempool and its gossip layer
    let mempool_network = spawn_mempool_network_actor(&cfg, &private_key, &registry, &span).await;
    let mempool = spawn_mempool_actor(mempool_network, &cfg.mempool, &span).await;
    let mempool_load = spawn_mempool_load_actor(&cfg.mempool.load, mempool.clone(), &span).await;

    // Spawn consensus gossip
    let network = spawn_network_actor(&cfg, &private_key, &registry, &span).await;

    // Spawn the host actor
    let host = spawn_host_actor(
        &home_dir,
        &cfg,
        &address,
        &private_key,
        &initial_validator_set,
        mempool,
        mempool_load,
        network.clone(),
        app_metrics,
        &span,
    )
    .await;

    let sync = spawn_sync_actor(
        ctx,
        network.clone(),
        host.clone(),
        &cfg.value_sync,
        sync_metrics,
        &span,
    )
    .await;

    let wal = spawn_wal_actor(&ctx, ProtobufCodec, &home_dir, &registry, &span).await;

    // Spawn consensus
    let consensus = spawn_consensus_actor(
        start_height,
        initial_validator_set,
        address,
        ctx,
        cfg,
        signing_provider,
        network.clone(),
        host.clone(),
        wal.clone(),
        sync.clone(),
        consensus_metrics,
        tx_event,
        &span,
    )
    .await;

    // Spawn the node actor
    let node = Node::new(ctx, network, consensus, wal, sync, host, span);

    let (actor_ref, handle) = node.spawn().await.unwrap();

    (actor_ref, handle)
}

async fn spawn_wal_actor(
    ctx: &MockContext,
    codec: ProtobufCodec,
    home_dir: &Path,
    registry: &SharedRegistry,
    span: &tracing::Span,
) -> WalRef<MockContext> {
    let wal_dir = home_dir.join("wal");
    std::fs::create_dir_all(&wal_dir).unwrap();
    let wal_file = wal_dir.join("consensus.wal");

    Wal::spawn(ctx, codec, wal_file, registry.clone(), span.clone())
        .await
        .unwrap()
}

async fn spawn_sync_actor(
    ctx: MockContext,
    network: NetworkRef<MockContext>,
    host: HostRef<MockContext>,
    config: &ValueSyncConfig,
    sync_metrics: sync::Metrics,
    span: &tracing::Span,
) -> Option<SyncRef<MockContext>> {
    if !config.enabled {
        return None;
    }

    let params = SyncParams {
        status_update_interval: config.status_update_interval,
        request_timeout: config.request_timeout,
    };

    let scoring_strategy = match config.scoring_strategy {
        config::ScoringStrategy::Ema => sync::scoring::Strategy::Ema,
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

    let actor_ref = Sync::spawn(
        ctx,
        network,
        host,
        params,
        sync_config,
        sync_metrics,
        span.clone(),
    )
    .await
    .unwrap();

    Some(actor_ref)
}

#[allow(clippy::too_many_arguments)]
async fn spawn_consensus_actor(
    initial_height: Height,
    initial_validator_set: ValidatorSet,
    address: Address,
    ctx: MockContext,
    mut cfg: Config,
    signing_provider: Ed25519Provider,
    network: NetworkRef<MockContext>,
    host: HostRef<MockContext>,
    wal: WalRef<MockContext>,
    sync: Option<SyncRef<MockContext>>,
    consensus_metrics: ConsensusMetrics,
    tx_event: TxEvent<MockContext>,
    span: &tracing::Span,
) -> ConsensusRef<MockContext> {
    let consensus_params = ConsensusParams {
        initial_height,
        initial_validator_set,
        address,
        threshold_params: Default::default(),
        value_payload: ValuePayload::PartsOnly,
    };

    // Derive the consensus queue capacity from `sync.parallel_requests` and `sync.batch_size`
    cfg.consensus.queue_capacity = cfg.value_sync.parallel_requests * cfg.value_sync.batch_size;

    Consensus::spawn(
        ctx,
        consensus_params,
        cfg.consensus,
        Box::new(signing_provider),
        network,
        host,
        wal,
        sync,
        consensus_metrics,
        tx_event,
        span.clone(),
    )
    .await
    .unwrap()
}

async fn spawn_network_actor(
    cfg: &Config,
    private_key: &PrivateKey,
    registry: &SharedRegistry,
    span: &tracing::Span,
) -> NetworkRef<MockContext> {
    use malachitebft_network as gossip;

    let bootstrap_protocol = match cfg.consensus.p2p.discovery.bootstrap_protocol {
        config::BootstrapProtocol::Kademlia => gossip::BootstrapProtocol::Kademlia,
        config::BootstrapProtocol::Full => gossip::BootstrapProtocol::Full,
    };

    let selector = match cfg.consensus.p2p.discovery.selector {
        config::Selector::Kademlia => gossip::Selector::Kademlia,
        config::Selector::Random => gossip::Selector::Random,
    };

    let config_gossip = gossip::Config {
        listen_addr: cfg.consensus.p2p.listen_addr.clone(),
        persistent_peers: cfg.consensus.p2p.persistent_peers.clone(),
        discovery: gossip::DiscoveryConfig {
            enabled: cfg.consensus.p2p.discovery.enabled,
            bootstrap_protocol,
            selector,
            num_outbound_peers: cfg.consensus.p2p.discovery.num_outbound_peers,
            num_inbound_peers: cfg.consensus.p2p.discovery.num_inbound_peers,
            max_connections_per_peer: cfg.consensus.p2p.discovery.max_connections_per_peer,
            ephemeral_connection_timeout: cfg.consensus.p2p.discovery.ephemeral_connection_timeout,
            ..Default::default()
        },
        idle_connection_timeout: Duration::from_secs(15 * 60),
        transport: gossip::TransportProtocol::from_multiaddr(&cfg.consensus.p2p.listen_addr)
            .unwrap_or_else(|| {
                panic!(
                    "No valid transport protocol found in listen address: {}",
                    cfg.consensus.p2p.listen_addr
                )
            }),
        pubsub_protocol: match cfg.consensus.p2p.protocol {
            config::PubSubProtocol::GossipSub(_) => gossip::PubSubProtocol::GossipSub,
            config::PubSubProtocol::Broadcast => gossip::PubSubProtocol::Broadcast,
        },
        gossipsub: match cfg.consensus.p2p.protocol {
            config::PubSubProtocol::GossipSub(config) => gossip::GossipSubConfig {
                mesh_n: config.mesh_n(),
                mesh_n_high: config.mesh_n_high(),
                mesh_n_low: config.mesh_n_low(),
                mesh_outbound_min: config.mesh_outbound_min(),
            },
            config::PubSubProtocol::Broadcast => gossip::GossipSubConfig::default(),
        },
        channel_names: ChannelNames::default(),
        rpc_max_size: cfg.consensus.p2p.rpc_max_size.as_u64() as usize,
        pubsub_max_size: cfg.consensus.p2p.pubsub_max_size.as_u64() as usize,
        enable_sync: true,
        protocol_names: gossip::ProtocolNames {
            consensus: cfg.consensus.p2p.protocol_names.consensus.clone(),
            discovery_kad: cfg.consensus.p2p.protocol_names.discovery_kad.clone(),
            discovery_regres: cfg.consensus.p2p.protocol_names.discovery_regres.clone(),
            sync: cfg.consensus.p2p.protocol_names.sync.clone(),
        },
    };

    let keypair = make_keypair(private_key);
    let codec = ProtobufCodec;

    Network::spawn(
        keypair,
        config_gossip,
        registry.clone(),
        codec,
        span.clone(),
    )
    .await
    .unwrap()
}

fn make_keypair(pk: &PrivateKey) -> Keypair {
    Keypair::ed25519_from_bytes(pk.inner().to_bytes()).unwrap()
}

async fn spawn_mempool_actor(
    mempool_network: MempoolNetworkRef,
    mempool_config: &MempoolConfig,
    span: &tracing::Span,
) -> MempoolRef {
    Mempool::spawn(
        mempool_network,
        mempool_config.gossip_batch_size,
        mempool_config.max_tx_count,
        span.clone(),
    )
    .await
    .unwrap()
}

async fn spawn_mempool_load_actor(
    mempool_load_config: &MempoolLoadConfig,
    mempool: MempoolRef,
    span: &tracing::Span,
) -> MempoolLoadRef {
    MempoolLoad::spawn(
        Params {
            load_type: mempool_load_config.load_type.clone(),
        },
        mempool,
        span.clone(),
    )
    .await
    .unwrap()
}

async fn spawn_mempool_network_actor(
    cfg: &Config,
    private_key: &PrivateKey,
    registry: &SharedRegistry,
    span: &tracing::Span,
) -> MempoolNetworkRef {
    let keypair = make_keypair(private_key);

    let config = MempoolNetworkConfig {
        listen_addr: cfg.mempool.p2p.listen_addr.clone(),
        persistent_peers: cfg.mempool.p2p.persistent_peers.clone(),
        idle_connection_timeout: Duration::from_secs(15 * 60),
    };

    MempoolNetwork::spawn(keypair, config, registry.clone(), span.clone())
        .await
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
async fn spawn_host_actor(
    home_dir: &Path,
    cfg: &Config,
    address: &Address,
    private_key: &PrivateKey,
    initial_validator_set: &ValidatorSet,
    mempool: MempoolRef,
    mempool_load: MempoolLoadRef,
    network: NetworkRef<MockContext>,
    metrics: AppMetrics,
    span: &tracing::Span,
) -> HostRef<MockContext> {
    if cfg.consensus.value_payload != config::ValuePayload::PartsOnly {
        warn!(
            "`value_payload` must be set to `PartsOnly` for Starknet app, ignoring current configuration `{:?}`",
            cfg.consensus.value_payload
        );
    }

    let mock_params = StarknetParams {
        max_block_size: cfg.test.max_block_size,
        txs_per_part: cfg.test.txs_per_part,
        time_allowance_factor: cfg.test.time_allowance_factor,
        exec_time_per_tx: cfg.test.exec_time_per_tx,
        max_retain_blocks: cfg.test.max_retain_blocks,
        stable_block_times: cfg.test.stable_block_times,
    };

    let mock_host = StarknetHost::new(
        mock_params,
        mempool.clone(),
        mempool_load.clone(),
        *address,
        private_key.clone(),
        initial_validator_set.clone(),
    );

    Host::spawn(
        home_dir.to_owned(),
        mock_host,
        mempool,
        mempool_load,
        network,
        metrics,
        span.clone(),
    )
    .await
    .unwrap()
}
