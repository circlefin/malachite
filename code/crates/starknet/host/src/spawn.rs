use std::path::{Path, PathBuf};
use std::time::Duration;

use libp2p_identity::ecdsa;
use malachite_actors::util::events::TxEvent;
use malachite_actors::wal::{Wal, WalRef};
use tokio::task::JoinHandle;

use malachite_actors::consensus::{Consensus, ConsensusParams, ConsensusRef};
use malachite_actors::gossip_consensus::{GossipConsensus, GossipConsensusRef};
use malachite_actors::host::HostRef;
use malachite_actors::node::{Node, NodeRef};
use malachite_actors::sync::{Params as SyncParams, Sync, SyncRef};
use malachite_config::{
    self as config, Config as NodeConfig, MempoolConfig, SyncConfig, TestConfig, TransportProtocol,
};
use malachite_consensus::ValuePayload;
use malachite_gossip_consensus::{
    Config as GossipConsensusConfig, DiscoveryConfig, GossipSubConfig, Keypair, PubSubProtocol,
};
use malachite_metrics::Metrics;
use malachite_metrics::SharedRegistry;
use malachite_sync as sync;
use malachite_test_mempool::Config as GossipMempoolConfig;

use crate::actor::Host;
use crate::codec::ProtobufCodec;
use crate::gossip_mempool::{GossipMempool, GossipMempoolRef};
use crate::host::{StarknetHost, StarknetParams};
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
    let gossip_mempool = spawn_gossip_mempool_actor(&cfg, &private_key, &registry, &span).await;
    let mempool = spawn_mempool_actor(gossip_mempool.clone(), &cfg.mempool, &cfg.test, &span).await;

    // Spawn consensus gossip
    let gossip_consensus = spawn_gossip_consensus_actor(&cfg, &private_key, &registry, &span).await;

    // Spawn the host actor
    let host = spawn_host_actor(
        &home_dir,
        &cfg,
        &address,
        &private_key,
        &initial_validator_set,
        mempool.clone(),
        gossip_consensus.clone(),
        metrics.clone(),
        &span,
    )
    .await;

    let sync = spawn_sync_actor(
        ctx.clone(),
        gossip_consensus.clone(),
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
        gossip_consensus.clone(),
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
        gossip_consensus,
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
    gossip_consensus: GossipConsensusRef<MockContext>,
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
    let actor_ref = Sync::spawn(ctx, gossip_consensus, host, params, metrics, span.clone())
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
    gossip_consensus: GossipConsensusRef<MockContext>,
    host: HostRef<MockContext>,
    wal: WalRef<MockContext>,
    sync: Option<SyncRef<MockContext>>,
    metrics: Metrics,
    tx_event: TxEvent<MockContext>,
    span: &tracing::Span,
) -> ConsensusRef<MockContext> {
    let value_payload = match cfg.consensus.value_payload {
        malachite_config::ValuePayload::PartsOnly => ValuePayload::PartsOnly,
        malachite_config::ValuePayload::ProposalOnly => ValuePayload::ProposalOnly,
        malachite_config::ValuePayload::ProposalAndParts => ValuePayload::ProposalAndParts,
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
        gossip_consensus,
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

async fn spawn_gossip_consensus_actor(
    cfg: &NodeConfig,
    private_key: &PrivateKey,
    registry: &SharedRegistry,
    span: &tracing::Span,
) -> GossipConsensusRef<MockContext> {
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
            config::PubSubProtocol::GossipSub(_) => PubSubProtocol::GossipSub,
            config::PubSubProtocol::Broadcast => PubSubProtocol::Broadcast,
        },
        gossipsub: match cfg.consensus.p2p.protocol {
            config::PubSubProtocol::GossipSub(config) => GossipSubConfig {
                mesh_n: config.mesh_n(),
                mesh_n_high: config.mesh_n_high(),
                mesh_n_low: config.mesh_n_low(),
                mesh_outbound_min: config.mesh_outbound_min(),
            },
            config::PubSubProtocol::Broadcast => GossipSubConfig::default(),
        },
        rpc_max_size: cfg.consensus.p2p.rpc_max_size.as_u64() as usize,
        pubsub_max_size: cfg.consensus.p2p.pubsub_max_size.as_u64() as usize,
    };

    let keypair = make_keypair(private_key);
    let codec = ProtobufCodec;

    GossipConsensus::spawn(
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
    gossip_mempool: GossipMempoolRef,
    mempool_config: &MempoolConfig,
    test_config: &TestConfig,
    span: &tracing::Span,
) -> MempoolRef {
    Mempool::spawn(
        gossip_mempool,
        mempool_config.clone(),
        *test_config,
        span.clone(),
    )
    .await
    .unwrap()
}

async fn spawn_gossip_mempool_actor(
    cfg: &NodeConfig,
    private_key: &PrivateKey,
    registry: &SharedRegistry,
    span: &tracing::Span,
) -> GossipMempoolRef {
    let config_gossip_mempool = GossipMempoolConfig {
        listen_addr: cfg.mempool.p2p.listen_addr.clone(),
        persistent_peers: cfg.mempool.p2p.persistent_peers.clone(),
        idle_connection_timeout: Duration::from_secs(15 * 60),
        transport: match cfg.mempool.p2p.transport {
            TransportProtocol::Tcp => malachite_test_mempool::TransportProtocol::Tcp,
            TransportProtocol::Quic => malachite_test_mempool::TransportProtocol::Quic,
        },
    };

    let keypair = make_keypair(private_key);
    GossipMempool::spawn(
        keypair,
        config_gossip_mempool,
        registry.clone(),
        span.clone(),
    )
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
    gossip_consensus: GossipConsensusRef<MockContext>,
    metrics: Metrics,
    span: &tracing::Span,
) -> HostRef<MockContext> {
    let value_payload = match cfg.consensus.value_payload {
        malachite_config::ValuePayload::PartsOnly => ValuePayload::PartsOnly,
        malachite_config::ValuePayload::ProposalOnly => ValuePayload::ProposalOnly,
        malachite_config::ValuePayload::ProposalAndParts => ValuePayload::ProposalAndParts,
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
        address.clone(),
        *private_key,
        initial_validator_set.clone(),
    );

    Host::spawn(
        home_dir.to_owned(),
        mock_host,
        mempool,
        gossip_consensus,
        metrics,
        span.clone(),
    )
    .await
    .unwrap()
}
