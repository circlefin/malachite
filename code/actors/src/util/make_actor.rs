use std::sync::Arc;

use ractor::ActorRef;
use tokio::sync::mpsc;

use malachite_common::Round;
use malachite_gossip::Keypair;
use malachite_node::value_builder::test::TestValueBuilder;
use malachite_test::utils::RotateProposer;
use malachite_test::{Address, Height, PrivateKey, TestContext, ValidatorSet, Value};
use tokio::task::JoinHandle;

use crate::node::{Msg as NodeMsg, Params as NodeParams};
use crate::timers::Config as TimersConfig;

pub async fn make_node_actor(
    initial_validator_set: ValidatorSet,
    private_key: PrivateKey,
    address: Address,
    tx_decision: mpsc::Sender<(Height, Round, Value)>,
) -> (ActorRef<NodeMsg>, JoinHandle<()>) {
    let keypair = Keypair::ed25519_from_bytes(private_key.inner().to_bytes()).unwrap();
    let start_height = Height::new(1);
    let ctx = TestContext::new(private_key);
    let proposer_selector = Arc::new(RotateProposer);

    let value_builder = Box::<TestValueBuilder<TestContext>>::default();

    let timers_config = TimersConfig::default();

    let params = NodeParams {
        address,
        initial_validator_set,
        keypair,
        proposer_selector,
        start_height,
        threshold_params: Default::default(),
        timers_config,
        tx_decision,
        value_builder,
    };

    crate::node::spawn(ctx, params).await.unwrap()
}
