mod full_nodes;
mod liveness;
mod middlewares;
mod n3f0;
mod n3f0_consensus_mode;
mod n3f0_pubsub_protocol;
mod n3f1;
mod reset;
mod validator_set;
mod value_sync;
mod vote_rebroadcast;
mod wal;

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use informalsystems_malachitebft_test::middleware::Middleware;
use malachitebft_test_app::config::Config;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tempfile::TempDir;

use malachitebft_app::node::Node;
use malachitebft_signing_ed25519::PrivateKey;
use malachitebft_test_app::node::{App, Handle};
use malachitebft_test_framework::HasTestRunner;
use malachitebft_test_framework::{NodeRunner, TestNode};

pub use malachitebft_test_framework::TestBuilder as GenTestBuilder;
pub use malachitebft_test_framework::{HandlerResult, NodeId, TestParams};

use informalsystems_malachitebft_test::{Height, TestContext, Validator, ValidatorSet};

pub type TestBuilder<S> = GenTestBuilder<TestContext, S>;

impl HasTestRunner<TestRunner> for TestContext {
    type Runner = TestRunner;
}

#[derive(Clone)]
pub struct TestRunner {
    pub id: usize,
    pub params: TestParams,
    pub nodes_info: HashMap<NodeId, NodeInfo>,
    pub private_keys: HashMap<NodeId, PrivateKey>,
    pub validator_set: ValidatorSet,
    pub consensus_base_port: usize,
    pub mempool_base_port: usize,
    pub metrics_base_port: usize,
}

fn temp_dir(id: NodeId) -> PathBuf {
    TempDir::with_prefix(format!("malachitebft-test-app-{id}"))
        .unwrap()
        .keep()
}

#[derive(Clone)]
pub struct NodeInfo {
    start_height: Height,
    home_dir: PathBuf,
    middleware: Arc<dyn Middleware>,
}

#[async_trait]
impl NodeRunner<TestContext> for TestRunner {
    type NodeHandle = Handle;

    fn new<S>(id: usize, nodes: &[TestNode<TestContext, S>], params: TestParams) -> Self {
        let base_port = 20_000 + id * 1000;

        let (validators, private_keys) = make_validators(nodes);
        let validator_set = ValidatorSet::new(validators);

        let nodes_info = nodes
            .iter()
            .map(|node| {
                (
                    node.id,
                    NodeInfo {
                        start_height: node.start_height,
                        home_dir: temp_dir(node.id),
                        middleware: Arc::clone(&node.middleware),
                    },
                )
            })
            .collect();

        Self {
            id,
            params,
            nodes_info,
            private_keys,
            validator_set,
            consensus_base_port: base_port,
            mempool_base_port: base_port + 100,
            metrics_base_port: base_port + 200,
        }
    }

    async fn spawn(&self, id: NodeId) -> eyre::Result<Handle> {
        let app = App {
            config: self.generate_config(id),
            home_dir: self.nodes_info[&id].home_dir.clone(),
            validator_set: self.validator_set.clone(),
            private_key: self.private_keys[&id].clone(),
            start_height: Some(self.nodes_info[&id].start_height),
            middleware: Some(Arc::clone(&self.nodes_info[&id].middleware)),
        };

        app.start().await
    }

    async fn reset_db(&self, id: NodeId) -> eyre::Result<()> {
        let db_dir = self.nodes_info[&id].home_dir.join("db");
        std::fs::remove_dir_all(&db_dir)?;
        std::fs::create_dir_all(&db_dir)?;
        Ok(())
    }
}

impl TestRunner {
    fn generate_config(&self, node: NodeId) -> Config {
        let mut config = self.generate_default_config(node);
        self.params.apply_to_config(&mut config);
        config
    }

    fn generate_default_config(&self, node: NodeId) -> Config {
        use malachitebft_config::*;

        let transport = transport_from_env(TransportProtocol::Tcp);
        let protocol = PubSubProtocol::default();

        let i = node - 1;

        Config {
            moniker: format!("node-{node}"),
            logging: LoggingConfig::default(),
            consensus: ConsensusConfig {
                // Current test app does not support proposal-only value payload properly as Init does not include valid_round
                value_payload: ValuePayload::ProposalAndParts,
                queue_capacity: 100, // Deprecated, derived from `sync.parallel_requests`
                timeouts: TimeoutConfig::default(),
                p2p: P2pConfig {
                    protocol,
                    discovery: DiscoveryConfig::default(),
                    listen_addr: transport.multiaddr("127.0.0.1", self.consensus_base_port + i),
                    persistent_peers: (0..self.nodes_info.len())
                        .filter(|j| i != *j)
                        .map(|j| transport.multiaddr("127.0.0.1", self.consensus_base_port + j))
                        .collect(),
                    ..Default::default()
                },
            },
            value_sync: ValueSyncConfig {
                enabled: true,
                status_update_interval: Duration::from_secs(2),
                request_timeout: Duration::from_secs(5),
                ..Default::default()
            },
            metrics: MetricsConfig {
                enabled: false,
                listen_addr: format!("127.0.0.1:{}", self.metrics_base_port + i)
                    .parse()
                    .unwrap(),
            },
            runtime: RuntimeConfig::single_threaded(),
            test: TestConfig::default(),
        }
    }
}

use malachitebft_config::TransportProtocol;

fn transport_from_env(default: TransportProtocol) -> TransportProtocol {
    if let Ok(protocol) = std::env::var("MALACHITE_TRANSPORT") {
        TransportProtocol::from_str(&protocol).unwrap_or(default)
    } else {
        default
    }
}

fn make_validators<S>(
    nodes: &[TestNode<TestContext, S>],
) -> (Vec<Validator>, HashMap<NodeId, PrivateKey>) {
    let mut rng = StdRng::seed_from_u64(0x42);

    let mut validators = Vec::new();
    let mut private_keys = HashMap::new();

    for node in nodes {
        let sk = PrivateKey::generate(&mut rng);
        let val = Validator::new(sk.public_key(), node.voting_power);

        private_keys.insert(node.id, sk);

        if node.voting_power > 0 {
            validators.push(val);
        }
    }

    (validators, private_keys)
}
