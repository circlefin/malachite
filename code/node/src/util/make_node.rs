use std::sync::Arc;
use std::time::Duration;

use malachite_test::utils::FixedProposer;
use malachite_test::{Address, Height, PrivateKey, TestContext, ValidatorSet};

use crate::network::broadcast;
use crate::network::gossip;
use crate::node::{Node, Params};
use crate::peers::Peers;
use crate::timers;
use crate::value::test::TestValueBuilder;

pub async fn make_broadcast_node(
    validator_set: ValidatorSet,
    private_key: PrivateKey,
    address: Address,
    peer_info: broadcast::PeerInfo,
    peers: Peers<TestContext>,
) -> Node<TestContext, broadcast::Handle> {
    let start_height = Height::new(1);
    let ctx = TestContext::new(private_key);
    let proposer_selector = Arc::new(FixedProposer::new(validator_set.validators[0].address));
    let proposal_builder = Arc::new(TestValueBuilder::default());

    let params = Params {
        start_height,
        proposer_selector,
        proposal_builder,
        validator_set,
        address,
        threshold_params: Default::default(),
    };

    let timers_config = timers::Config {
        propose_timeout: Duration::from_secs(3),
        prevote_timeout: Duration::from_secs(1),
        precommit_timeout: Duration::from_secs(1),
        commit_timeout: Duration::from_secs(1),
    };

    let network = broadcast::Peer::new(peer_info.clone());
    let handle = network.run().await;

    let timeout = Some(Duration::from_secs(30));

    let to_connect = peers
        .iter()
        .filter(|p| p.id != peer_info.id)
        .map(|p| p.peer_info());

    for peer in to_connect {
        handle.connect_to_peer(peer, timeout).await;
    }

    Node::new(ctx, params, handle, timers_config)
}

pub async fn make_gossip_node(
    validator_set: ValidatorSet,
    private_key: PrivateKey,
    address: Address,
) -> Node<TestContext, malachite_gossip::Handle> {
    let keypair = gossip::Keypair::ed25519_from_bytes(private_key.inner().to_bytes()).unwrap();

    let start_height = Height::new(1);
    let ctx = TestContext::new(private_key);
    let proposer_selector = Arc::new(FixedProposer::new(validator_set.validators[0].address));
    let proposal_builder = Arc::new(TestValueBuilder::default());

    let params = Params {
        start_height,
        proposer_selector,
        proposal_builder,
        validator_set,
        address,
        threshold_params: Default::default(),
    };

    let timers_config = timers::Config {
        propose_timeout: Duration::from_secs(3),
        prevote_timeout: Duration::from_secs(1),
        precommit_timeout: Duration::from_secs(1),
        commit_timeout: Duration::from_secs(1),
    };

    let config = malachite_gossip::Config::default();

    let addr = "/ip4/0.0.0.0/tcp/0".parse().unwrap();
    let handle = malachite_gossip::spawn(keypair, addr, config)
        .await
        .unwrap();

    Node::new(ctx, params, handle, timers_config)
}
