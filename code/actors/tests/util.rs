#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

use malachite_common::{Round, VotingPower};
use malachite_test::utils::make_validators;
use malachite_test::{Height, PrivateKey, Validator, ValidatorSet, Value};

use malachite_actors::node::Msg;
use malachite_actors::util::make_node_actor;

pub const SEED: u64 = 42;
pub const HEIGHTS: u64 = 3;
pub const START_HEIGHT: Height = Height::new(1);
pub const END_HEIGHT: Height = Height::new(START_HEIGHT.as_u64() + HEIGHTS - 1);
pub const TEST_TIMEOUT: Duration = Duration::from_secs(20);

pub struct Test<const N: usize> {
    pub nodes: [TestNode; N],
    pub validator_set: ValidatorSet,
    pub vals_and_keys: [(Validator, PrivateKey); N],
    pub expected_decisions: usize,
}

impl<const N: usize> Test<N> {
    pub fn new(nodes: [TestNode; N], expected_decisions: usize) -> Self {
        let vals_and_keys = make_validators(Self::voting_powers(&nodes));
        let validators = vals_and_keys.iter().map(|(v, _)| v).cloned();
        let validator_set = ValidatorSet::new(validators);

        Self {
            nodes,
            validator_set,
            vals_and_keys,
            expected_decisions,
        }
    }

    pub fn get(&self, index: usize) -> Option<&TestNode> {
        self.nodes.get(index)
    }

    pub fn voting_powers(nodes: &[TestNode; N]) -> [VotingPower; N] {
        let mut voting_powers = [0; N];
        for (i, node) in nodes.iter().enumerate() {
            voting_powers[i] = node.voting_power;
        }
        voting_powers
    }
}

pub struct TestNode {
    pub voting_power: VotingPower,
}

impl TestNode {
    pub fn correct(voting_power: VotingPower) -> Self {
        Self { voting_power }
    }
}

pub async fn run_test<const N: usize>(test: Test<N>) {
    tracing_subscriber::fmt::init();

    let mut handles = Vec::with_capacity(N);

    for (v, sk) in &test.vals_and_keys {
        let (tx_decision, rx_decision) = mpsc::channel(HEIGHTS as usize);

        let node = tokio::spawn(make_node_actor(
            test.validator_set.clone(),
            sk.clone(),
            v.address,
            tx_decision,
        ));

        handles.push((node, rx_decision));
    }

    sleep(Duration::from_secs(5)).await;

    let mut nodes = Vec::with_capacity(handles.len());
    for (handle, rx) in handles {
        let node = handle.await.expect("Error: node failed to start");
        nodes.push((node, rx));
    }

    let mut actors = Vec::with_capacity(nodes.len());
    let mut rxs = Vec::with_capacity(nodes.len());

    for ((actor, _), rx) in nodes {
        actor.cast(Msg::Start).unwrap();

        actors.push(actor);
        rxs.push(rx);
    }

    let correct_decisions = Arc::new(AtomicUsize::new(0));

    for (i, mut rx_decision) in rxs.into_iter().enumerate() {
        let i = i + 1;

        let correct_decisions = Arc::clone(&correct_decisions);

        tokio::spawn(async move {
            for height in START_HEIGHT.as_u64()..=END_HEIGHT.as_u64() {
                let decision = rx_decision.recv().await;
                let expected = Some((Height::new(height), Round::new(0), Value::new(40 + height)));

                if decision == expected {
                    info!("[{height}] {i}/{HEIGHTS} correct decision");
                    correct_decisions.fetch_add(1, Ordering::Relaxed);
                } else {
                    error!("[{height}] {i}/{HEIGHTS} incorrect decision: expected {expected:?}, got {decision:?}");
                }
            }
        });
    }

    tokio::time::sleep(TEST_TIMEOUT).await;

    let correct_decisions = correct_decisions.load(Ordering::Relaxed);

    if correct_decisions != test.expected_decisions {
        panic!(
            "Not all nodes made correct decisions: {}/{}",
            correct_decisions, test.expected_decisions
        );
    }

    for actor in actors {
        actor.stop_and_wait(None, None).await.unwrap();
    }
}
