#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{error, info};

use malachite_common::{Round, VotingPower};
use malachite_test::utils::{make_mempool_nodes, make_validators, make_value};
use malachite_test::{Height, PrivateKey, Validator, ValidatorSet};

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
    pub mempool_nodes: [PrivateKey; N],
    pub expected_decisions: usize,
}

impl<const N: usize> Test<N> {
    pub fn new(nodes: [TestNode; N], expected_decisions: usize) -> Self {
        let voting_powers = Self::voting_powers(&nodes);
        let vals_and_keys = make_validators(voting_powers);
        let validators = vals_and_keys.iter().map(|(v, _)| v).cloned();
        let validator_set = ValidatorSet::new(validators);
        let mempool_nodes = make_mempool_nodes();

        Self {
            nodes,
            validator_set,
            vals_and_keys,
            mempool_nodes,
            expected_decisions,
        }
    }

    pub fn voting_powers(nodes: &[TestNode; N]) -> [VotingPower; N] {
        let mut voting_powers = [0; N];
        for (i, node) in nodes.iter().enumerate() {
            voting_powers[i] = node.voting_power;
        }
        voting_powers
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    NoStart,
    Crash(u64),
}

#[derive(Clone)]
pub struct TestNode {
    pub voting_power: VotingPower,
    pub faults: Vec<Fault>,
}

impl TestNode {
    pub fn correct(voting_power: VotingPower) -> Self {
        Self {
            voting_power,
            faults: vec![],
        }
    }

    pub fn faulty(voting_power: VotingPower, faults: Vec<Fault>) -> Self {
        Self {
            voting_power,
            faults,
        }
    }

    fn start_node(&self) -> bool {
        !self.faults.contains(&Fault::NoStart)
    }

    fn crashes_at(&self, height: u64) -> bool {
        self.faults.iter().any(|f| match f {
            Fault::NoStart => false,
            Fault::Crash(h) => *h == height,
        })
    }
}

fn init_logging() {
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::FmtSubscriber;

    let builder = FmtSubscriber::builder()
        .with_target(false)
        .with_env_filter("malachite=trace")
        .with_thread_ids(false);

    let subscriber = builder.finish();
    subscriber.init();
}

pub async fn run_test<const N: usize>(test: Test<N>) {
    init_logging();

    let mut handles = Vec::with_capacity(N);

    let val_keys: Vec<PrivateKey> = test
        .vals_and_keys
        .iter()
        .map(|(_, pk)| pk.clone())
        .collect();

    for i in 0..N {
        if test.nodes[i].faults.contains(&Fault::NoStart) {
            continue;
        }
        let (v, sk) = &test.vals_and_keys[i];
        let (tx_decision, rx_decision) = mpsc::channel(HEIGHTS as usize);
        let node_sk = &test.mempool_nodes[i];
        let node = tokio::spawn(make_node_actor(
            test.validator_set.clone(),
            val_keys.clone(),
            sk.clone(),
            test.mempool_nodes.to_vec(),
            node_sk.clone(),
            v.address,
            tx_decision,
        ));

        handles.push((node, rx_decision));
    }

    let mut nodes = Vec::with_capacity(handles.len());
    for (i, (handle, rx)) in handles.into_iter().enumerate() {
        let (actor_ref, _) = handle.await.expect("Error: node failed to start");
        let test = test.nodes[i].clone();
        nodes.push((actor_ref, test, rx));
    }

    let mut actors = Vec::with_capacity(nodes.len());
    let mut rxs = Vec::with_capacity(nodes.len());

    for (actor, _, rx) in nodes {
        actors.push(actor);
        rxs.push(rx);
    }

    let correct_decisions = Arc::new(AtomicUsize::new(0));

    for (i, mut rx_decision) in rxs.into_iter().enumerate() {
        let correct_decisions = Arc::clone(&correct_decisions);

        let node_test = test.nodes[i].clone();
        let actor_ref = actors[i].clone();

        tokio::spawn(async move {
            for height in START_HEIGHT.as_u64()..=END_HEIGHT.as_u64() {
                if node_test.crashes_at(height) {
                    info!("[{i}] Faulty node {i} has crashed");
                    actor_ref.kill();
                    break;
                }

                let decision = rx_decision.recv().await;
                // TODO - the value proposed comes from a set of mempool Tx-es which are currently different for each proposer
                // Also heights can go to higher rounds.
                // Therefore removing the round and value check for now
                let value = make_value([40 + height]);

                let expected = Some((Height::new(height), Round::new(0), value));

                match decision {
                    Some((h, _r, v)) if h == Height::new(height) && !v.is_empty() => {
                        info!("[{i}] {height}/{HEIGHTS} correct decision");
                        correct_decisions.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => {
                        error!("[{i}] {height}/{HEIGHTS} incorrect decision: expected {expected:?}, got {decision:?}")
                    }
                }
            }
        });
    }

    tokio::time::sleep(TEST_TIMEOUT).await;

    let correct_decisions = correct_decisions.load(Ordering::Relaxed);

    if correct_decisions != test.expected_decisions {
        panic!(
            "Not all nodes made correct decisions: got {}, expected {}",
            correct_decisions, test.expected_decisions
        );
    }

    for actor in actors {
        let _ = actor.stop_and_wait(None, None).await;
    }
}
