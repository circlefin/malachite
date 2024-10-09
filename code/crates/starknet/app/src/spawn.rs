use std::time::Duration;

use libp2p_identity::ecdsa;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use malachite_actors::consensus::{Consensus, ConsensusParams, ConsensusRef};
use malachite_actors::gossip_consensus::{GossipConsensus, GossipConsensusRef};
use malachite_actors::gossip_mempool::{GossipMempool, GossipMempoolRef};
use malachite_actors::host::HostRef;
use malachite_actors::node::{Node, NodeRef};
use malachite_common::Round;
use malachite_gossip_consensus::{Config as GossipConsensusConfig, Keypair};
use malachite_gossip_mempool::Config as GossipMempoolConfig;
use malachite_metrics::Metrics;
use malachite_metrics::SharedRegistry;
use malachite_node::config::{
    Config as NodeConfig, MempoolConfig, PubSubProtocol, TestConfig, TransportProtocol,
};
use malachite_starknet_host::actor::StarknetHost;
use malachite_starknet_host::mempool::{Mempool, MempoolRef};
use malachite_starknet_host::mock::context::MockContext;
use malachite_starknet_host::mock::host::{MockHost, MockParams};
use malachite_starknet_host::types::{Address, BlockHash, Height, PrivateKey, ValidatorSet};

use crate::codec::ProtobufCodec;

pub async fn spawn_node_actor(
    cfg: NodeConfig,
    initial_validator_set: ValidatorSet,
    private_key: PrivateKey,
    tx_decision: Option<mpsc::Sender<(Height, Round, BlockHash)>>,
) -> (NodeRef, JoinHandle<()>) {
    let ctx = MockContext::new(private_key);

    let registry = SharedRegistry::global();
    let metrics = Metrics::register(registry);
    let address = Address::from_public_key(private_key.public_key());

    // Spawn mempool and its gossip layer
    let gossip_mempool = spawn_gossip_mempool_actor(&cfg, &private_key, registry).await;
    let mempool = spawn_mempool_actor(gossip_mempool.clone(), &cfg.mempool, &cfg.test).await;

    // Spawn consensus gossip
    let gossip_consensus = spawn_gossip_consensus_actor(&cfg, &private_key, registry).await;

    // Spawn the host actor
    let host = spawn_host_actor(
        &cfg,
        &address,
        &initial_validator_set,
        mempool.clone(),
        gossip_consensus.clone(),
        metrics.clone(),
    )
    .await;

    let start_height = Height::new(1);

    // Spawn consensus
    let consensus = spawn_consensus_actor(
        start_height,
        initial_validator_set,
        address,
        ctx.clone(),
        cfg,
        gossip_consensus.clone(),
        host.clone(),
        metrics,
        tx_decision,
    )
    .await;

    // Spawn the node actor
    let node = Node::new(
        ctx,
        gossip_consensus,
        consensus,
        gossip_mempool,
        mempool.get_cell(),
        host,
        start_height,
    );

    let (actor_ref, handle) = node.spawn().await.unwrap();

    (actor_ref, handle)
}

#[allow(clippy::too_many_arguments)]
async fn spawn_consensus_actor(
    start_height: Height,
    initial_validator_set: ValidatorSet,
    address: Address,
    ctx: MockContext,
    cfg: NodeConfig,
    gossip_consensus: GossipConsensusRef<MockContext>,
    host: HostRef<MockContext>,
    metrics: Metrics,
    tx_decision: Option<mpsc::Sender<(Height, Round, BlockHash)>>,
) -> ConsensusRef<MockContext> {
    let consensus_params = ConsensusParams {
        start_height,
        initial_validator_set,
        address,
        threshold_params: Default::default(),
    };

    Consensus::spawn(
        ctx,
        consensus_params,
        cfg.consensus.timeouts,
        gossip_consensus,
        host,
        metrics,
        tx_decision,
    )
    .await
    .unwrap()
}

async fn spawn_gossip_consensus_actor(
    cfg: &NodeConfig,
    private_key: &PrivateKey,
    registry: &SharedRegistry,
) -> GossipConsensusRef<MockContext> {
    let config_gossip = GossipConsensusConfig {
        listen_addr: cfg.consensus.p2p.listen_addr.clone(),
        persistent_peers: cfg.consensus.p2p.persistent_peers.clone(),
        idle_connection_timeout: Duration::from_secs(60),
        transport: match cfg.consensus.p2p.transport {
            TransportProtocol::Tcp => malachite_gossip_consensus::TransportProtocol::Tcp,
            TransportProtocol::Quic => malachite_gossip_consensus::TransportProtocol::Quic,
        },
        protocol: match cfg.consensus.p2p.protocol {
            PubSubProtocol::GossipSub => malachite_gossip_consensus::PubSubProtocol::GossipSub,
            PubSubProtocol::Broadcast => malachite_gossip_consensus::PubSubProtocol::Broadcast,
        },
    };

    let keypair = make_keypair(private_key);
    let codec = ProtobufCodec;

    GossipConsensus::spawn(keypair, config_gossip, registry.clone(), codec)
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
) -> MempoolRef {
    Mempool::spawn(gossip_mempool, mempool_config, test_config)
        .await
        .unwrap()
}

async fn spawn_gossip_mempool_actor(
    cfg: &NodeConfig,
    private_key: &PrivateKey,
    registry: &SharedRegistry,
) -> GossipMempoolRef {
    let config_gossip_mempool = GossipMempoolConfig {
        listen_addr: cfg.mempool.p2p.listen_addr.clone(),
        persistent_peers: cfg.mempool.p2p.persistent_peers.clone(),
        idle_connection_timeout: Duration::from_secs(60),
        transport: match cfg.mempool.p2p.transport {
            TransportProtocol::Tcp => malachite_gossip_mempool::TransportProtocol::Tcp,
            TransportProtocol::Quic => malachite_gossip_mempool::TransportProtocol::Quic,
        },
    };

    let keypair = make_keypair(private_key);
    GossipMempool::spawn(keypair, config_gossip_mempool, registry.clone())
        .await
        .unwrap()
}

async fn spawn_host_actor(
    cfg: &NodeConfig,
    address: &Address,
    initial_validator_set: &ValidatorSet,
    mempool: MempoolRef,
    gossip_consensus: GossipConsensusRef<MockContext>,
    metrics: Metrics,
) -> HostRef<MockContext> {
    let mock_params = MockParams {
        max_block_size: cfg.consensus.max_block_size,
        tx_size: cfg.test.tx_size,
        txs_per_part: cfg.test.txs_per_part,
        time_allowance_factor: cfg.test.time_allowance_factor,
        exec_time_per_tx: cfg.test.exec_time_per_tx,
    };

    let mock_host = MockHost::new(
        mock_params,
        mempool.clone(),
        address.clone(),
        initial_validator_set.clone(),
    );

    StarknetHost::spawn(mock_host, mempool, gossip_consensus, metrics)
        .await
        .unwrap()
}
