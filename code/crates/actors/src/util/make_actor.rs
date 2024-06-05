use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use malachite_common::Round;
use malachite_gossip_consensus::{Config as GossipConsensusConfig, Keypair};
use malachite_gossip_mempool::Config as GossipMempoolConfig;
use malachite_node::config::Config as NodeConfig;
use malachite_test::{Address, Height, PrivateKey, TestContext, ValidatorSet, Value};

use crate::consensus::{Consensus, ConsensusParams, ConsensusRef};
use crate::gossip_consensus::{GossipConsensus, GossipConsensusRef};
use crate::gossip_mempool::{GossipMempool, GossipMempoolRef};
use crate::host::{Host, HostRef};
use crate::mempool::{Mempool, MempoolRef};
use crate::node::{Node, NodeRef};
use crate::util::value_builder::test::TestParams as TestValueBuilderParams;
use crate::util::PartStore;
use crate::util::TestValueBuilder;

pub async fn spawn_node_actor(
    cfg: NodeConfig,
    initial_validator_set: ValidatorSet,
    validator_pk: PrivateKey,
    node_pk: PrivateKey,
    address: Address,
    tx_decision: mpsc::Sender<(Height, Round, Value)>,
) -> (NodeRef, JoinHandle<()>) {
    let ctx = TestContext::new(validator_pk.clone());

    // Spawn mempool and its gossip layer
    let gossip_mempool = spawn_gossip_mempool_actor(&cfg, node_pk).await;
    let mempool = spawn_mempool_actor(gossip_mempool.clone()).await;

    // Configure the value builder
    let value_builder = make_test_value_builder(mempool.clone(), &cfg);

    // Spawn the host actor
    let host = spawn_host_actor(value_builder, &initial_validator_set).await;

    // Spawn consensus and its gossip
    let gossip_consensus = spawn_gossip_consensus_actor(&cfg, validator_pk).await;

    let start_height = Height::new(1);

    let consensus = spawn_consensus_actor(
        start_height,
        initial_validator_set,
        address,
        ctx.clone(),
        cfg,
        gossip_consensus.clone(),
        host.clone(),
        tx_decision,
    )
    .await;

    // Spawn the node actor
    let node = Node::new(
        ctx,
        gossip_consensus,
        consensus,
        gossip_mempool,
        mempool,
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
    ctx: TestContext,
    cfg: NodeConfig,
    gossip_consensus: GossipConsensusRef,
    host: HostRef<TestContext>,
    tx_decision: mpsc::Sender<(Height, Round, Value)>,
) -> ConsensusRef<TestContext> {
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
        tx_decision,
        None,
    )
    .await
    .unwrap()
}

async fn spawn_gossip_consensus_actor(
    cfg: &NodeConfig,
    validator_pk: PrivateKey,
) -> GossipConsensusRef {
    let config_gossip = GossipConsensusConfig {
        listen_addr: cfg.consensus.p2p.listen_addr.clone(),
        persistent_peers: cfg.consensus.p2p.persistent_peers.clone(),
        idle_connection_timeout: Duration::from_secs(60),
    };

    let validator_keypair = Keypair::ed25519_from_bytes(validator_pk.inner().to_bytes()).unwrap();

    GossipConsensus::spawn(validator_keypair.clone(), config_gossip, None)
        .await
        .unwrap()
}

async fn spawn_host_actor(
    value_builder: TestValueBuilder<TestContext>,
    initial_validator_set: &ValidatorSet,
) -> HostRef<TestContext> {
    Host::spawn(
        Box::new(value_builder),
        PartStore::new(),
        initial_validator_set.clone(),
    )
    .await
    .unwrap()
}

fn make_test_value_builder(mempool: MempoolRef, cfg: &NodeConfig) -> TestValueBuilder<TestContext> {
    TestValueBuilder::new(
        mempool,
        TestValueBuilderParams {
            max_block_size: cfg.consensus.max_block_size,
            tx_size: cfg.test.tx_size,
            txs_per_part: cfg.test.txs_per_part,
            time_allowance_factor: cfg.test.time_allowance_factor,
            exec_time_per_part: cfg.test.exec_time_per_part,
        },
    )
}

async fn spawn_mempool_actor(gossip_mempool: GossipMempoolRef) -> MempoolRef {
    Mempool::spawn(gossip_mempool, None).await.unwrap()
}

async fn spawn_gossip_mempool_actor(cfg: &NodeConfig, node_pk: PrivateKey) -> GossipMempoolRef {
    let config_gossip_mempool = GossipMempoolConfig {
        listen_addr: cfg.mempool.p2p.listen_addr.clone(),
        persistent_peers: cfg.mempool.p2p.persistent_peers.clone(),
        idle_connection_timeout: Duration::from_secs(60),
    };

    let node_keypair = Keypair::ed25519_from_bytes(node_pk.inner().to_bytes()).unwrap();

    GossipMempool::spawn(node_keypair.clone(), config_gossip_mempool, None)
        .await
        .unwrap()
}
