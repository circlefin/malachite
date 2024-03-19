use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use malachite_common::Round;
use malachite_node::network::gossip;
use malachite_node::node::Params;
use malachite_node::timers;
use malachite_node::value::test::TestValueBuilder;
use malachite_test::utils::RotateProposer;
use malachite_test::{Address, Height, PrivateKey, TestContext, ValidatorSet, Value};

use crate::gossip::Gossip;
use crate::node::Node;
use crate::proposal_builder::ProposalBuilder;

pub async fn make_node_actor(
    validator_set: ValidatorSet,
    private_key: PrivateKey,
    address: Address,
    tx_decision: mpsc::Sender<(Height, Round, Value)>,
) -> Node<TestContext> {
    let keypair = gossip::Keypair::ed25519_from_bytes(private_key.inner().to_bytes()).unwrap();
    let start_height = Height::new(1);
    let ctx = TestContext::new(private_key);
    let proposer_selector = Arc::new(RotateProposer);

    let (proposal_builder, _) =
        ProposalBuilder::<TestContext, _>::spawn(TestValueBuilder::<TestContext>::default())
            .await
            .unwrap();

    let params = Params {
        start_height,
        proposer_selector,
        proposal_builder: Arc::new(TestValueBuilder::default()), // unused
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

    let addr = "/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap();
    let (gossip, _) = Gossip::spawn(keypair, addr, config).await.unwrap();

    Node::new(
        ctx,
        params,
        timers_config,
        gossip,
        proposal_builder,
        tx_decision,
    )
}
