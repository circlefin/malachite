use ractor::ActorRef;
use std::hash::{DefaultHasher, Hasher};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use malachite_common::Round;
use malachite_gossip::Keypair;
use malachite_gossip_mempool::Multiaddr;
use malachite_test::{Address, Height, PrivateKey, TestContext, ValidatorSet, Value};

use crate::consensus::Consensus;
use crate::gossip::Gossip;
use crate::gossip_mempool::GossipMempool;
use crate::host::Host;
use crate::mempool::Mempool;
use crate::node::{Msg as NodeMsg, Msg, Node};
use crate::timers::Config as TimersConfig;
use crate::util::PartStore;
use crate::util::TestValueBuilder;

pub async fn make_node_actor(
    initial_validator_set: ValidatorSet,
    validator_pk: PrivateKey,
    node_pk: PrivateKey,
    address: Address,
    tx_decision: mpsc::Sender<(Height, Round, Value)>,
) -> (ActorRef<NodeMsg>, JoinHandle<()>) {
    // Hash validator_pk into a number between 27000 and 28000 for port
    let pk_bytes = validator_pk.inner().to_bytes();
    let mut hasher = DefaultHasher::new();
    hasher.write_u64(u64::from_le_bytes(pk_bytes[0..8].try_into().unwrap()));
    let port: u64 = hasher.finish() % 1000 + 27000;

    let addr: Multiaddr = format!("/ip4/127.0.0.1/udp/{}/quic-v1", port)
        .parse()
        .unwrap();
    let mempool_addr: Multiaddr = format!("/ip4/127.0.0.1/udp/{}/quic-v1", port + 1)
        .parse()
        .unwrap();

    // Spawn mempool and its gossip
    let config_gossip_mempool = malachite_gossip_mempool::Config::default();
    let node_keypair = Keypair::ed25519_from_bytes(node_pk.inner().to_bytes()).unwrap();

    let gossip_mempool = GossipMempool::spawn(
        node_keypair.clone(),
        mempool_addr,
        config_gossip_mempool,
        None,
    )
    .await
    .unwrap();

    let mempool = Mempool::spawn(crate::mempool::Params {}, gossip_mempool.clone(), None)
        .await
        .unwrap();

    let ctx = TestContext::new(validator_pk.clone());

    // Spawn the host actor
    let value_builder = Box::new(TestValueBuilder::<TestContext>::new(mempool.clone()));
    let host = Host::spawn(
        value_builder,
        PartStore::new(),
        initial_validator_set.clone(),
    )
    .await
    .unwrap();

    // Spawn consensus and its gossip
    let validator_keypair = Keypair::ed25519_from_bytes(validator_pk.inner().to_bytes()).unwrap();

    let config_gossip = malachite_gossip::Config::default();

    let gossip_consensus = Gossip::spawn(validator_keypair.clone(), addr, config_gossip, None)
        .await
        .unwrap();

    let timers_config = TimersConfig::default();

    let start_height = Height::new(1);

    let consensus_params = crate::consensus::Params {
        start_height,
        initial_validator_set,
        address,
        threshold_params: Default::default(),
    };

    let consensus = Consensus::spawn(
        ctx.clone(),
        consensus_params,
        timers_config,
        gossip_consensus.clone(),
        host.clone(),
        tx_decision,
        None,
    )
    .await
    .unwrap();

    // Spawn the node actor
    let node = Node::new(
        ctx,
        gossip_consensus,
        consensus.clone(),
        gossip_mempool,
        mempool,
        host,
        start_height,
    );

    let result = node.spawn().await.unwrap();
    let actor = result.0.clone();
    let _ = actor.cast(Msg::Start);

    result
}
