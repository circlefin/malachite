use ractor::ActorRef;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use malachite_common::Round;
use malachite_gossip::{Keypair, PeerId};
use malachite_gossip_mempool::Multiaddr;
use malachite_test::{Address, Height, PrivateKey, TestContext, ValidatorSet, Value};

use crate::cal::CAL;
use crate::consensus::Consensus;
use crate::gossip::Gossip;
use crate::gossip_mempool::GossipMempool;
use crate::mempool::Mempool;
use crate::node::{Msg as NodeMsg, Node};
use crate::proposal_builder::ProposalBuilder;
use crate::timers::Config as TimersConfig;
use crate::util::TestValueBuilder;

pub async fn make_node_actor(
    initial_validator_set: ValidatorSet,
    validator_pks: Vec<PrivateKey>,
    validator_pk: PrivateKey,
    nodes_pks: Vec<PrivateKey>,
    node_pk: PrivateKey,
    address: Address,
    tx_decision: mpsc::Sender<(Height, Round, Value)>,
) -> (ActorRef<NodeMsg>, JoinHandle<()>) {
    let addr: Multiaddr = "/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap();

    // Spawn mempool and its gossip
    let config_gossip_mempool = malachite_gossip_mempool::Config::default();
    let node_keypair = Keypair::ed25519_from_bytes(node_pk.inner().to_bytes()).unwrap();

    let node_keypairs: Vec<Keypair> = nodes_pks
        .iter()
        .map(|pk| Keypair::ed25519_from_bytes(pk.inner().to_bytes()).unwrap())
        .collect();
    let node_peer_ids = node_keypairs
        .iter()
        .map(|pk| PeerId::from_public_key(&pk.public()))
        .collect();

    let gossip_mempool = GossipMempool::spawn(
        node_keypair.clone(),
        addr.clone(),
        node_peer_ids,
        config_gossip_mempool,
        None,
    )
    .await
    .unwrap();

    let mempool = Mempool::spawn(crate::mempool::Params {}, gossip_mempool.clone(), None)
        .await
        .unwrap();

    // Spawn the proposal builder
    let mut builder = TestValueBuilder::<TestContext>::new(mempool.clone(), None);
    let value_builder = Box::new(builder.clone());

    let ctx = TestContext::new(validator_pk.clone());
    let proposal_builder = ProposalBuilder::spawn(ctx.clone(), value_builder)
        .await
        .unwrap();

    // Spawn the CAL actor
    let cal = CAL::spawn(ctx.clone(), initial_validator_set.clone())
        .await
        .unwrap();

    // Spawn consensus and its gossip
    let validator_keypair = Keypair::ed25519_from_bytes(validator_pk.inner().to_bytes()).unwrap();

    let validator_keypairs: Vec<Keypair> = validator_pks
        .iter()
        .map(|pk| Keypair::ed25519_from_bytes(pk.inner().to_bytes()).unwrap())
        .collect();
    let validator_peer_ids: Vec<PeerId> = validator_keypairs
        .iter()
        .map(|pk| PeerId::from_public_key(&pk.public()))
        .collect();

    let config_gossip = malachite_gossip::Config::default();

    let gossip_consensus = Gossip::spawn(
        validator_keypair.clone(),
        addr,
        validator_peer_ids.clone(),
        config_gossip,
        None,
    )
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
        cal.clone(),
        proposal_builder.clone(),
        tx_decision,
        None,
    )
    .await
    .unwrap();

    // ??? circular dependencies - no effect since proposal builder is using a clone
    builder.batch_gossip = Some(consensus.clone());

    // Spawn the node actor
    Node::new(
        ctx,
        cal,
        gossip_consensus,
        consensus.clone(),
        gossip_mempool,
        mempool,
        proposal_builder,
        start_height,
    )
    .spawn()
    .await
    .unwrap()
}
