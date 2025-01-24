use std::path::{Path, PathBuf};
use std::time::Duration;

use libp2p_identity::ecdsa;
use malachitebft_engine::util::events::TxEvent;
use malachitebft_engine::wal::{Wal, WalRef};
use tokio::task::JoinHandle;

use malachitebft_config::{
    self as config, Config as NodeConfig, MempoolConfig, SyncConfig, TestConfig, TransportProtocol,
};
use malachitebft_core_consensus::ValuePayload;
use malachitebft_engine::consensus::{Consensus, ConsensusParams, ConsensusRef};
use malachitebft_engine::host::HostRef;
use malachitebft_engine::network::{Network, NetworkRef};
use malachitebft_engine::node::{Node, NodeRef};
use malachitebft_engine::sync::{Params as SyncParams, Sync, SyncRef};
use malachitebft_metrics::Metrics;
use malachitebft_metrics::SharedRegistry;
use malachitebft_network::Keypair;
use malachitebft_sync as sync;
use malachitebft_test_mempool::Config as MempoolNetworkConfig;

use crate::actor::Host;
use crate::codec::ProtobufCodec;
use crate::host::{StarknetHost, StarknetParams};
use crate::mempool::network::{MempoolNetwork, MempoolNetworkRef};
use crate::mempool::{Mempool, MempoolRef};
use crate::types::MockContext;
use crate::types::{Address, Height, PrivateKey, ValidatorSet};

pub async fn spawn_node_actor(
    cfg: NodeConfig,
    home_dir: PathBuf,
    initial_validator_set: ValidatorSet,
    private_key: PrivateKey,
    start_height: Option<Height>,
    tx_event: TxEvent<MockContext>,
    span: tracing::Span,
) -> (NodeRef, JoinHandle<()>) {
    let ctx = MockContext::new(private_key);

    let start_height = start_height.unwrap_or(Height::new(1, 1));

    let registry = SharedRegistry::global().with_moniker(cfg.moniker.as_str());
    let metrics = Metrics::register(&registry);
    let address = Address::from_public_key(private_key.public_key());

    // Spawn mempool and its gossip layer
    let mempool_network = spawn_mempool_network_actor(&cfg, &private_key, &registry, &span).await;
    let mempool =
        spawn_mempool_actor(mempool_network.clone(), &cfg.mempool, &cfg.test, &span).await;

    // Spawn consensus gossip
    let network = spawn_network_actor(&cfg, &private_key, &registry, &span).await;

    // Spawn the host actor
    let host = spawn_host_actor(
        &home_dir,
        &cfg,
        &address,
        &private_key,
        &initial_validator_set,
        mempool.clone(),
        network.clone(),
        metrics.clone(),
        &span,
    )
    .await;

    let sync = spawn_sync_actor(
        ctx.clone(),
        network.clone(),
        host.clone(),
        &cfg.sync,
        &registry,
        &span,
    )
    .await;

    let wal = spawn_wal_actor(&ctx, ProtobufCodec, &home_dir, &registry, &span).await;

    // Spawn consensus
    let consensus = spawn_consensus_actor(
        start_height,
        initial_validator_set,
        address,
        ctx.clone(),
        cfg,
        network.clone(),
        host.clone(),
        wal.clone(),
        sync.clone(),
        metrics,
        tx_event,
        &span,
    )
    .await;

    // Spawn the node actor
    let node = Node::new(
        ctx,
        network,
        consensus,
        wal,
        sync,
        mempool.get_cell(),
        host,
        start_height,
        span,
    );

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
    config: &SyncConfig,
    registry: &SharedRegistry,
    span: &tracing::Span,
) -> Option<SyncRef<MockContext>> {
    if !config.enabled {
        return None;
    }

    let params = SyncParams {
        status_update_interval: config.status_update_interval,
        request_timeout: config.request_timeout,
    };

    let metrics = sync::Metrics::register(registry);
    let actor_ref = Sync::spawn(ctx, network, host, params, metrics, span.clone())
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
    cfg: NodeConfig,
    network: NetworkRef<MockContext>,
    host: HostRef<MockContext>,
    wal: WalRef<MockContext>,
    sync: Option<SyncRef<MockContext>>,
    metrics: Metrics,
    tx_event: TxEvent<MockContext>,
    span: &tracing::Span,
) -> ConsensusRef<MockContext> {
    let value_payload = match cfg.consensus.value_payload {
        malachitebft_config::ValuePayload::PartsOnly => ValuePayload::PartsOnly,
        malachitebft_config::ValuePayload::ProposalOnly => ValuePayload::ProposalOnly,
        malachitebft_config::ValuePayload::ProposalAndParts => ValuePayload::ProposalAndParts,
    };

    let consensus_params = ConsensusParams {
        initial_height,
        initial_validator_set,
        address,
        threshold_params: Default::default(),
        value_payload,
    };

    Consensus::spawn(
        ctx,
        consensus_params,
        cfg.consensus.timeouts,
        network,
        host,
        wal,
        sync,
        metrics,
        tx_event,
        span.clone(),
    )
    .await
    .unwrap()
}

async fn spawn_network_actor(
    cfg: &NodeConfig,
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
            ephemeral_connection_timeout: cfg.consensus.p2p.discovery.ephemeral_connection_timeout,
            ..Default::default()
        },
        idle_connection_timeout: Duration::from_secs(15 * 60),
        transport: match cfg.consensus.p2p.transport {
            TransportProtocol::Tcp => gossip::TransportProtocol::Tcp,
            TransportProtocol::Quic => gossip::TransportProtocol::Quic,
        },
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
        rpc_max_size: cfg.consensus.p2p.rpc_max_size.as_u64() as usize,
        pubsub_max_size: cfg.consensus.p2p.pubsub_max_size.as_u64() as usize,
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

fn make_keypair(private_key: &PrivateKey) -> Keypair {
    let pk_bytes = private_key.inner().to_bytes_be();
    let secret_key = ecdsa::SecretKey::try_from_bytes(pk_bytes).unwrap();
    let ecdsa_keypair = ecdsa::Keypair::from(secret_key);
    Keypair::from(ecdsa_keypair)
}

async fn spawn_mempool_actor(
    mempool_network: MempoolNetworkRef,
    mempool_config: &MempoolConfig,
    test_config: &TestConfig,
    span: &tracing::Span,
) -> MempoolRef {
    Mempool::spawn(
        mempool_network,
        mempool_config.gossip_batch_size,
        mempool_config.max_tx_count,
        test_config.tx_size,
        span.clone(),
    )
    .await
    .unwrap()
}

async fn spawn_mempool_network_actor(
    cfg: &NodeConfig,
    private_key: &PrivateKey,
    registry: &SharedRegistry,
    span: &tracing::Span,
) -> MempoolNetworkRef {
    let keypair = make_keypair(private_key);

    let config = MempoolNetworkConfig {
        listen_addr: cfg.mempool.p2p.listen_addr.clone(),
        persistent_peers: cfg.mempool.p2p.persistent_peers.clone(),
        idle_connection_timeout: Duration::from_secs(15 * 60),
        transport: match cfg.mempool.p2p.transport {
            TransportProtocol::Tcp => malachitebft_test_mempool::TransportProtocol::Tcp,
            TransportProtocol::Quic => malachitebft_test_mempool::TransportProtocol::Quic,
        },
    };

    MempoolNetwork::spawn(keypair, config, registry.clone(), span.clone())
        .await
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
async fn spawn_host_actor(
    home_dir: &Path,
    cfg: &NodeConfig,
    address: &Address,
    private_key: &PrivateKey,
    initial_validator_set: &ValidatorSet,
    mempool: MempoolRef,
    network: NetworkRef<MockContext>,
    metrics: Metrics,
    span: &tracing::Span,
) -> HostRef<MockContext> {
    let value_payload = match cfg.consensus.value_payload {
        malachitebft_config::ValuePayload::PartsOnly => ValuePayload::PartsOnly,
        malachitebft_config::ValuePayload::ProposalOnly => ValuePayload::ProposalOnly,
        malachitebft_config::ValuePayload::ProposalAndParts => ValuePayload::ProposalAndParts,
    };

    let mock_params = StarknetParams {
        value_payload,
        max_block_size: cfg.consensus.max_block_size,
        tx_size: cfg.test.tx_size,
        txs_per_part: cfg.test.txs_per_part,
        time_allowance_factor: cfg.test.time_allowance_factor,
        exec_time_per_tx: cfg.test.exec_time_per_tx,
        max_retain_blocks: cfg.test.max_retain_blocks,
        vote_extensions: cfg.test.vote_extensions,
    };

    let mock_host = StarknetHost::new(
        mock_params,
        mempool.clone(),
        *address,
        *private_key,
        initial_validator_set.clone(),
    );

    Host::spawn(
        home_dir.to_owned(),
        mock_host,
        mempool,
        network,
        metrics,
        span.clone(),
    )
    .await
    .unwrap()
}
