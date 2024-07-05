use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use malachite_actors::consensus::Metrics;
use malachite_actors::gossip_mempool::GossipMempoolRef;
use malachite_actors::mempool::MempoolRef;
use malachite_actors::node::{Node, NodeRef};
use malachite_common::Round;
use malachite_metrics::SharedRegistry;
use malachite_node::config::{Config as NodeConfig, MempoolConfig, TestConfig};
use malachite_starknet_app::spawn::{
    spawn_consensus_actor, spawn_gossip_consensus_actor, spawn_gossip_mempool_actor,
    spawn_host_actor,
};
use malachite_starknet_host::mock::context::MockContext;
use malachite_starknet_host::mock::types::{
    Address, Height, PrivateKey, ProposalContent, ValidatorSet,
};

use crate::kvmempool::KvMempool;

// Shamelessly reuse as much as we can from the Starknet app/host/context
// The only difference is in using `spawn_kvmempool_actor`
pub async fn spawn_node_actor_kv(
    cfg: NodeConfig,
    initial_validator_set: ValidatorSet,
    validator_pk: PrivateKey,
    node_pk: PrivateKey,
    address: Address,
    tx_decision: Option<mpsc::Sender<(Height, Round, ProposalContent)>>,
) -> (NodeRef, JoinHandle<()>) {
    let ctx = MockContext::new(validator_pk.clone());

    // Set up the metrics along with the Prometheus client registry
    let registry = SharedRegistry::global();
    let metrics = Metrics::register(registry);

    // Spawn mempool and its gossip layer
    let gossip_mempool = spawn_gossip_mempool_actor(&cfg, node_pk, registry).await;
    let mempool = spawn_kvmempool_actor(gossip_mempool.clone(), &cfg.mempool, &cfg.test).await;

    // Spawn the host actor
    let host = spawn_host_actor(
        &cfg,
        &initial_validator_set,
        mempool.clone(),
        metrics.clone(),
    )
    .await;

    // Spawn consensus and its gossip
    let gossip_consensus = spawn_gossip_consensus_actor(&cfg, validator_pk, registry).await;

    let start_height = Height::new(1);

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
        mempool,
        host,
        start_height,
    );

    let (actor_ref, handle) = node.spawn().await.unwrap();

    (actor_ref, handle)
}

async fn spawn_kvmempool_actor(
    gossip_mempool: GossipMempoolRef,
    mempool_config: &MempoolConfig,
    test_config: &TestConfig,
) -> MempoolRef {
    KvMempool::spawn(gossip_mempool, mempool_config, test_config, None)
        .await
        .unwrap()
}
