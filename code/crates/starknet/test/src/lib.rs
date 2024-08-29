use core::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{error, info, Instrument};

use malachite_common::VotingPower;
use malachite_node::config::{Config as NodeConfig, LoggingConfig, PubSubProtocol};
use malachite_starknet_app::spawn::spawn_node_actor;

use malachite_starknet_host::types::{Height, PrivateKey, Validator, ValidatorSet};

pub use malachite_node::config::App;

pub enum Expected {
    Exactly(usize),
    AtLeast(usize),
    AtMost(usize),
    LessThan(usize),
    GreaterThan(usize),
}

impl Expected {
    pub fn check(&self, actual: usize) -> bool {
        match self {
            Expected::Exactly(expected) => actual == *expected,
            Expected::AtLeast(expected) => actual >= *expected,
            Expected::AtMost(expected) => actual <= *expected,
            Expected::LessThan(expected) => actual < *expected,
            Expected::GreaterThan(expected) => actual > *expected,
        }
    }
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expected::Exactly(n) => write!(f, "exactly {n}"),
            Expected::AtLeast(n) => write!(f, "at least {n}"),
            Expected::AtMost(n) => write!(f, "at most {n}"),
            Expected::LessThan(n) => write!(f, "less than {n}"),
            Expected::GreaterThan(n) => write!(f, "greater than {n}"),
        }
    }
}

pub struct Test<const N: usize> {
    pub nodes: [TestNode; N],
    pub validator_set: ValidatorSet,
    pub vals_and_keys: [(Validator, PrivateKey); N],
    pub expected_decisions: Expected,
    pub consensus_base_port: usize,
    pub mempool_base_port: usize,
    pub metrics_base_port: usize,
}

impl<const N: usize> Test<N> {
    pub fn new(nodes: [TestNode; N], expected_decisions: Expected) -> Self {
        let vals_and_keys = make_validators(Self::voting_powers(&nodes));
        let validators = vals_and_keys.iter().map(|(v, _)| v).cloned();
        let validator_set = ValidatorSet::new(validators);

        Self {
            nodes,
            validator_set,
            vals_and_keys,
            expected_decisions,
            consensus_base_port: rand::thread_rng().gen_range(21000..30000),
            mempool_base_port: rand::thread_rng().gen_range(31000..40000),
            metrics_base_port: rand::thread_rng().gen_range(41000..50000),
        }
    }

    pub fn voting_powers(nodes: &[TestNode; N]) -> [VotingPower; N] {
        let mut voting_powers = [0; N];
        for (i, node) in nodes.iter().enumerate() {
            voting_powers[i] = node.voting_power;
        }
        voting_powers
    }

    pub async fn run(self, app: App) {
        init_logging();

        let mut handles = Vec::with_capacity(N);

        for i in 0..N {
            if self.nodes[i].faults.contains(&Fault::NoStart) {
                continue;
            }

            let (_, private_key) = &self.vals_and_keys[i];
            let (tx_decision, rx_decision) = mpsc::channel(HEIGHTS as usize);

            let node_config = make_node_config(&self, i, app);

            let node = tokio::spawn(spawn_node_actor(
                node_config,
                self.validator_set.clone(),
                *private_key,
                Some(tx_decision),
            ));

            handles.push((node, rx_decision));
        }

        sleep(Duration::from_secs(5)).await;

        let mut nodes = Vec::with_capacity(handles.len());
        for (i, (handle, rx)) in handles.into_iter().enumerate() {
            let (actor_ref, _) = handle.await.expect("Error: node failed to start");
            let test = self.nodes[i].clone();
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

            let node_test = self.nodes[i].clone();
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

                        // TODO: Heights can go to higher rounds, therefore removing the round and value check for now.
                        match decision {
                            Some((h, _r, _)) if h.as_u64() == height /* && r == Round::new(0) */ => {
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

        if !self.expected_decisions.check(correct_decisions) {
            panic!(
                "Incorrect number of decisions: got {}, expected {}",
                correct_decisions, self.expected_decisions
            );
        }

        for actor in actors {
            let _ = actor.stop_and_wait(None, None).await;
        }
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

    pub fn start_node(&self) -> bool {
        !self.faults.contains(&Fault::NoStart)
    }

    pub fn crashes_at(&self, height: u64) -> bool {
        self.faults.iter().any(|f| match f {
            Fault::NoStart => false,
            Fault::Crash(h) => *h == height,
        })
    }
}

pub const HEIGHTS: u64 = 3;
pub const START_HEIGHT: Height = Height::new(1);
pub const END_HEIGHT: Height = Height::new(START_HEIGHT.as_u64() + HEIGHTS - 1);
pub const TEST_TIMEOUT: Duration = Duration::from_secs(20);

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

use bytesize::ByteSize;

use malachite_node::config::{
    ConsensusConfig, MempoolConfig, MetricsConfig, P2pConfig, RuntimeConfig, TimeoutConfig,
};

pub fn make_node_config<const N: usize>(test: &Test<N>, i: usize, app: App) -> NodeConfig {
    NodeConfig {
        app,
        moniker: format!("node-{i}"),
        logging: LoggingConfig::default(),
        consensus: ConsensusConfig {
            max_block_size: ByteSize::mib(1),
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                protocol: PubSubProtocol::GossipSub,
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
                protocol: PubSubProtocol::GossipSub,
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
        metrics: MetricsConfig {
            enabled: false,
            listen_addr: format!("127.0.0.1:{}", test.metrics_base_port + i)
                .parse()
                .unwrap(),
        },
        runtime: RuntimeConfig::single_threaded(),
        test: Default::default(),
    }
}

pub fn make_validators<const N: usize>(
    voting_powers: [VotingPower; N],
) -> [(Validator, PrivateKey); N] {
    let mut rng = StdRng::seed_from_u64(0x42);

    let mut validators = Vec::with_capacity(N);

    for vp in voting_powers {
        let sk = PrivateKey::generate(&mut rng);
        let val = Validator::new(sk.public_key(), vp);
        validators.push((val, sk));
    }

    validators.try_into().expect("N validators")
}
