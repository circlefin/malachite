#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use bytesize::ByteSize;
use rand::Rng;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{error, info, Instrument};

use malachite_common::{Round, VotingPower};
use malachite_node::config::{ConsensusConfig, MempoolConfig, P2pConfig, TimeoutConfig};
use malachite_test::utils::make_validators;
use malachite_test::{Height, PrivateKey, Validator, ValidatorSet};

use malachite_actors::util::spawn_node_actor;

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
    pub consensus_base_port: usize,
    pub mempool_base_port: usize,
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
            consensus_base_port: rand::thread_rng().gen_range(20000..30000),
            mempool_base_port: rand::thread_rng().gen_range(30000..40000),
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
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    let filter = EnvFilter::builder()
        .parse("info,malachite=debug,ractor=error")
        .unwrap();

    pub fn enable_ansi() -> bool {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
    }

    // Construct a tracing subscriber with the supplied filter and enable reloading.
    let builder = FmtSubscriber::builder()
        .with_target(false)
        .with_env_filter(filter)
        .with_writer(std::io::stdout)
        .with_ansi(enable_ansi())
        .with_thread_ids(false);

    let subscriber = builder.finish();
    subscriber.init();
}

pub async fn run_test<const N: usize>(test: Test<N>) {
    init_logging();

    let mut handles = Vec::with_capacity(N);

    for i in 0..N {
        if test.nodes[i].faults.contains(&Fault::NoStart) {
            continue;
        }

        let (v, sk) = &test.vals_and_keys[i];
        let (tx_decision, rx_decision) = mpsc::channel(HEIGHTS as usize);

        let node_config = make_node_config(&test, i);

        let node = tokio::spawn(spawn_node_actor(
            node_config,
            test.validator_set.clone(),
            sk.clone(),
            sk.clone(),
            v.address,
            tx_decision,
        ));

        handles.push((node, rx_decision));
    }

    sleep(Duration::from_secs(5)).await;

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

        tokio::spawn(
            async move {
                for height in START_HEIGHT.as_u64()..=END_HEIGHT.as_u64() {
                    if node_test.crashes_at(height) {
                        info!("Faulty node has crashed");
                        actor_ref.kill();
                        break;
                    }

                    let decision = rx_decision.recv().await;

                    // TODO - the value proposed comes from a set of mempool Tx-es which are currently different for each proposer
                    // Also heights can go to higher rounds.
                    // Therefore removing the round and value check for now
                    match decision {
                        Some((h, r, _)) if h == Height::new(height) && r == Round::new(0) => {
                            info!("{height}/{HEIGHTS} correct decision");
                            correct_decisions.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => {
                            error!("{height}/{HEIGHTS} no decision")
                        }
                    }
                }
            }
            .instrument(tracing::error_span!("node", i)),
        );
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

fn make_node_config<const N: usize>(test: &Test<N>, i: usize) -> malachite_node::config::Config {
    malachite_node::config::Config {
        moniker: format!("node-{i}"),
        consensus: ConsensusConfig {
            max_block_size: ByteSize::mib(1),
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                listen_addr: format!(
                    "/ip4/127.0.0.1/udp/{}/quic-v1",
                    test.consensus_base_port + i
                )
                .parse()
                .unwrap(),
                persistent_peers: (0..N)
                    .filter(|j| i != *j)
                    .map(|j| {
                        format!(
                            "/ip4/127.0.0.1/udp/{}/quic-v1",
                            test.consensus_base_port + j
                        )
                        .parse()
                        .unwrap()
                    })
                    .collect(),
            },
        },
        mempool: MempoolConfig {
            p2p: P2pConfig {
                listen_addr: format!("/ip4/127.0.0.1/udp/{}/quic-v1", test.mempool_base_port + i)
                    .parse()
                    .unwrap(),
                persistent_peers: (0..N)
                    .filter(|j| i != *j)
                    .map(|j| {
                        format!("/ip4/127.0.0.1/udp/{}/quic-v1", test.mempool_base_port + j)
                            .parse()
                            .unwrap()
                    })
                    .collect(),
            },
            max_tx_count: 10000,
            gossip_batch_size: 100,
        },
        test: Default::default(),
    }
}
