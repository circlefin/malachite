use core::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use libp2p_identity::{ecdsa, PeerId};
use malachite_config::TransportProtocol;
use malachite_gossip_consensus::{spawn, Config, DiscoveryConfig, Keypair};
use malachite_metrics::SharedRegistry;
use malachite_starknet_host::types::PrivateKey;
use rand::{rngs::StdRng, Rng, SeedableRng};
use tokio::time::sleep;
use tracing::{debug, info};

//---------------------------------------------------------------------
// Expected primitives
//---------------------------------------------------------------------

#[derive(Debug)]
pub enum Expected {
    Exactly(Vec<usize>),
    AtLeast(Vec<usize>),
    AtMost(Vec<usize>),
    LessThan(Vec<usize>),
    GreaterThan(Vec<usize>),
}

impl Expected {
    pub fn check(&self, actual: Vec<usize>) {
        match self {
            Expected::Exactly(expected) => {
                assert_eq!(
                    actual.len(),
                    expected.len(),
                    "Expected length: {}, actual length: {}",
                    expected.len(),
                    actual.len()
                );
                for node in expected {
                    assert!(actual.contains(node), "Node {} not found", node);
                }
            }
            Expected::AtLeast(expected) => {
                assert!(
                    actual.len() >= expected.len(),
                    "Expected length: at least {}, actual length: {}",
                    expected.len(),
                    actual.len()
                );
                for node in expected {
                    assert!(actual.contains(node), "Node {} not found", node);
                }
            }
            Expected::AtMost(expected) => {
                assert!(
                    actual.len() <= expected.len(),
                    "Expected length: at most {}, actual length: {}",
                    expected.len(),
                    actual.len()
                );
                for node in actual {
                    assert!(expected.contains(&node), "Node {} not expected", node);
                }
            }
            Expected::LessThan(expected) => {
                assert!(
                    actual.len() < expected.len(),
                    "Expected length: less than {}, actual length: {}",
                    expected.len(),
                    actual.len()
                );
                for node in actual {
                    assert!(expected.contains(&node), "Node {} not expected", node);
                }
            }
            Expected::GreaterThan(expected) => {
                assert!(
                    actual.len() > expected.len(),
                    "Expected length: greater than {}, actual length: {}",
                    expected.len(),
                    actual.len()
                );
                for node in expected {
                    assert!(actual.contains(node), "Node {} not found", node);
                }
            }
        }
    }
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expected::Exactly(v) => write!(f, "exactly {:?}", v),
            Expected::AtLeast(v) => write!(f, "at least {:?}", v),
            Expected::AtMost(v) => write!(f, "at most {:?}", v),
            Expected::LessThan(v) => write!(f, "less than {:?}", v),
            Expected::GreaterThan(v) => write!(f, "greater than {:?}", v),
        }
    }
}

//---------------------------------------------------------------------
// Test
//---------------------------------------------------------------------

pub struct Test<const N: usize> {
    nodes: [TestNode; N],
    expected_peers_sets: [Expected; N],
    keypairs: [Keypair; N],
    consensus_base_port: usize,
    spawn_delay: Duration,
    timeout: Duration,
}

impl<const N: usize> Test<N> {
    pub fn new(
        nodes: [TestNode; N],
        expected_peers_sets: [Expected; N],
        spawn_delay: Duration,
        timeout: Duration,
    ) -> Self {
        Self {
            nodes,
            expected_peers_sets,
            keypairs: Self::create_keypairs(),
            consensus_base_port: rand::thread_rng().gen_range(21000..50000),
            spawn_delay,
            timeout,
        }
    }

    fn create_keypairs() -> [Keypair; N] {
        let mut rng = StdRng::seed_from_u64(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
        std::array::from_fn(|_| {
            let privkey = PrivateKey::generate(&mut rng);
            let pk_bytes = privkey.inner().to_bytes_be();
            let secret_key = ecdsa::SecretKey::try_from_bytes(pk_bytes).unwrap();
            let ecdsa_keypair = ecdsa::Keypair::from(secret_key);
            Keypair::from(ecdsa_keypair)
        })
    }

    fn generate_default_configs(&self) -> [Config; N] {
        std::array::from_fn(|i| Config {
            listen_addr: TransportProtocol::Quic
                .multiaddr("127.0.0.1", self.consensus_base_port + i),
            persistent_peers: self.nodes[i]
                .bootstrap_nodes
                .iter()
                .map(|j| {
                    TransportProtocol::Quic.multiaddr("127.0.0.1", self.consensus_base_port + *j)
                })
                .collect(),
            discovery: DiscoveryConfig {
                enabled: true,
                ..Default::default()
            },
            idle_connection_timeout: Duration::from_secs(60),
            transport: malachite_gossip_consensus::TransportProtocol::Quic,
            protocol: malachite_gossip_consensus::PubSubProtocol::default(),
        })
    }

    pub async fn run(self) {
        init_logging();
        info!("Starting test with {} nodes", N);

        let configs = self.generate_default_configs();
        debug!("Generated configs");

        let mut handles = Vec::with_capacity(N);

        for (i, config) in configs.iter().enumerate().take(N) {
            if self.nodes[i].start_node() {
                let handle = spawn(
                    self.keypairs[i].clone(),
                    config.clone(),
                    SharedRegistry::global().clone(),
                )
                .await
                .unwrap();

                handles.push(handle);
                debug!(id = %i, "Spawned node");
                sleep(self.spawn_delay).await;
            }
        }

        sleep(self.timeout).await;

        let mut tasks = Vec::with_capacity(N);

        for mut handle in handles {
            let task = tokio::spawn(async move {
                let mut peers = Vec::new();

                loop {
                    tokio::select! {
                        event = handle.recv() => {
                            match event {
                                Some(malachite_gossip_consensus::Event::PeerConnected(peer_id)) => {
                                    if !peers.contains(&peer_id) {
                                        peers.push(peer_id);
                                    }
                                }
                                Some(malachite_gossip_consensus::Event::PeerDisconnected(peer_id)) => {
                                    if let Some(pos) = peers.iter().position(|p| p == &peer_id) {
                                        peers.remove(pos);
                                    }
                                }
                                Some(_) => {}
                                None => break,
                            }
                        }
                        _ = sleep(Duration::from_secs(1)) => {
                            handle.shutdown().await.unwrap();
                            break;
                        }
                    }
                }

                peers
            });

            tasks.push(task);
        }

        let actuals: Vec<Vec<PeerId>> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .map(|res| res.unwrap())
            .collect();

        let peer_id_to_index = |peer_id: &PeerId| -> usize {
            self.keypairs
                .iter()
                .enumerate()
                .find_map(|(i, keypair)| {
                    if PeerId::from_public_key(&keypair.public()) == *peer_id {
                        Some(i)
                    } else {
                        None
                    }
                })
                .expect("Peer not found")
        };

        for (i, expected) in self.expected_peers_sets.iter().enumerate() {
            let actual = actuals[i].iter().map(peer_id_to_index).collect::<Vec<_>>();
            expected.check(actual);
        }
    }
}

//---------------------------------------------------------------------
// Test node
//---------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    NoStart,
}

#[derive(Clone, Debug)]
pub struct TestNode {
    _id: usize,
    bootstrap_nodes: Vec<usize>,
    faults: Vec<Fault>,
}

impl TestNode {
    pub fn correct(id: usize, bootstrap_nodes: Vec<usize>) -> Self {
        Self {
            _id: id,
            bootstrap_nodes,
            faults: Vec::new(),
        }
    }

    pub fn faulty(id: usize, bootstrap_nodes: Vec<usize>, faults: Vec<Fault>) -> Self {
        Self {
            _id: id,
            bootstrap_nodes,
            faults,
        }
    }

    pub fn bootstrap_nodes(&self) -> &[usize] {
        &self.bootstrap_nodes
    }

    pub fn start_node(&self) -> bool {
        !self.faults.contains(&Fault::NoStart)
    }
}

//---------------------------------------------------------------------
// Helpers
//---------------------------------------------------------------------

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
