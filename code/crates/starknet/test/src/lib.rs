use core::fmt;
use std::fs::{create_dir_all, remove_dir_all};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use eyre::bail;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::task::JoinSet;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, error_span, info, Instrument, Span};

use malachitebft_config::{
    Config as NodeConfig, Config, DiscoveryConfig, LoggingConfig, PubSubProtocol, SyncConfig,
    TestConfig, TransportProtocol,
};
use malachitebft_core_consensus::{SignedConsensusMsg, ValueToPropose};
use malachitebft_core_types::{SignedVote, VotingPower};
use malachitebft_engine::util::events::{Event, RxEvent, TxEvent};
use malachitebft_starknet_host::spawn::spawn_node_actor;
use malachitebft_starknet_host::types::MockContext;
use malachitebft_starknet_host::types::{Height, PrivateKey, Validator, ValidatorSet};

#[cfg(test)]
pub mod tests;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

pub struct TestParams {
    pub enable_sync: bool,
    pub protocol: PubSubProtocol,
    pub block_size: ByteSize,
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub vote_extensions: Option<ByteSize>,
    pub value_payload: ValuePayload,
    pub max_retain_blocks: usize,
    pub timeout_step: Duration,
}

impl Default for TestParams {
    fn default() -> Self {
        Self {
            enable_sync: false,
            protocol: PubSubProtocol::default(),
            block_size: ByteSize::mib(1),
            tx_size: ByteSize::kib(1),
            txs_per_part: 256,
            vote_extensions: None,
            value_payload: ValuePayload::default(),
            max_retain_blocks: 50,
            timeout_step: Duration::from_secs(30),
        }
    }
}

impl TestParams {
    fn apply_to_config(&self, config: &mut Config) {
        config.sync.enabled = self.enable_sync;
        config.consensus.p2p.protocol = self.protocol;
        config.consensus.max_block_size = self.block_size;
        config.consensus.value_payload = self.value_payload;
        config.test.tx_size = self.tx_size;
        config.test.txs_per_part = self.txs_per_part;
        config.test.vote_extensions.enabled = self.vote_extensions.is_some();
        config.test.vote_extensions.size = self.vote_extensions.unwrap_or_default();
        config.test.max_retain_blocks = self.max_retain_blocks;
        config.consensus.timeouts.timeout_step = self.timeout_step;
    }
}

pub enum Step<S> {
    Crash(Duration),
    ResetDb,
    Restart(Duration),
    WaitUntil(u64),
    OnEvent(EventHandler<S>),
    Expect(Expected),
    Success,
    Fail(String),
}

#[derive(Copy, Clone, Debug)]
pub enum HandlerResult {
    WaitForNextEvent,
    ContinueTest,
}

pub type EventHandler<S> =
    Box<dyn Fn(Event<MockContext>, &mut S) -> Result<HandlerResult, eyre::Report> + Send + Sync>;

pub type NodeId = usize;

pub struct TestNode<State = ()> {
    pub id: NodeId,
    pub voting_power: VotingPower,
    pub start_height: Height,
    pub start_delay: Duration,
    pub steps: Vec<Step<State>>,
    pub state: State,
}

impl<State> TestNode<State> {
    pub fn new(id: usize) -> Self
    where
        State: Default,
    {
        Self::new_with_state(id, State::default())
    }

    pub fn new_with_state(id: usize, state: State) -> Self {
        Self {
            id,
            voting_power: 1,
            start_height: Height::new(1, 1),
            start_delay: Duration::from_secs(0),
            steps: vec![],
            state,
        }
    }

    pub fn with_state(&mut self, state: State) -> &mut Self {
        self.state = state;
        self
    }

    pub fn with_voting_power(&mut self, power: VotingPower) -> &mut Self {
        self.voting_power = power;
        self
    }

    pub fn start(&mut self) -> &mut Self {
        self.start_at(1)
    }

    pub fn start_at(&mut self, height: u64) -> &mut Self {
        self.start_after(height, Duration::from_secs(0))
    }

    pub fn start_after(&mut self, height: u64, delay: Duration) -> &mut Self {
        self.start_height.block_number = height;
        self.start_delay = delay;
        self
    }

    pub fn crash(&mut self) -> &mut Self {
        self.steps.push(Step::Crash(Duration::from_secs(0)));
        self
    }

    pub fn crash_after(&mut self, duration: Duration) -> &mut Self {
        self.steps.push(Step::Crash(duration));
        self
    }

    pub fn reset_db(&mut self) -> &mut Self {
        self.steps.push(Step::ResetDb);
        self
    }

    pub fn restart_after(&mut self, delay: Duration) -> &mut Self {
        self.steps.push(Step::Restart(delay));
        self
    }

    pub fn wait_until(&mut self, height: u64) -> &mut Self {
        self.steps.push(Step::WaitUntil(height));
        self
    }

    pub fn on_event<F>(&mut self, on_event: F) -> &mut Self
    where
        F: Fn(Event<MockContext>, &mut State) -> Result<HandlerResult, eyre::Report>
            + Send
            + Sync
            + 'static,
    {
        self.steps.push(Step::OnEvent(Box::new(on_event)));
        self
    }

    pub fn expect_wal_replay(&mut self, at_height: u64) -> &mut Self {
        self.on_event(move |event, _| {
            let Event::WalReplayBegin(height, count) = event else {
                return Ok(HandlerResult::WaitForNextEvent);
            };

            info!("Replaying WAL at height {height} with {count} messages");

            if height.as_u64() != at_height {
                bail!("Unexpected WAL replay at height {height}, expected {at_height}")
            }

            Ok(HandlerResult::ContinueTest)
        })
    }

    pub fn expect_vote_set_request(&mut self, at_height: u64) -> &mut Self {
        self.on_event(move |event, _| {
            let Event::RequestedVoteSet(height, round) = event else {
                return Ok(HandlerResult::WaitForNextEvent);
            };

            info!("Requested vote set for height {height} and round {round}");

            if height.as_u64() != at_height {
                bail!("Unexpected vote set request for height {height}, expected {at_height}")
            }

            Ok(HandlerResult::ContinueTest)
        })
    }

    pub fn on_proposed_value<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(ValueToPropose<MockContext>, &mut State) -> Result<HandlerResult, eyre::Report>
            + Send
            + Sync
            + 'static,
    {
        self.on_event(move |event, state| {
            if let Event::ProposedValue(value) = event {
                f(value, state)
            } else {
                Ok(HandlerResult::WaitForNextEvent)
            }
        })
    }

    pub fn on_vote<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(SignedVote<MockContext>, &mut State) -> Result<HandlerResult, eyre::Report>
            + Send
            + Sync
            + 'static,
    {
        self.on_event(move |event, state| {
            if let Event::Published(SignedConsensusMsg::Vote(vote)) = event {
                f(vote, state)
            } else {
                Ok(HandlerResult::WaitForNextEvent)
            }
        })
    }

    pub fn expect_decisions(&mut self, expected: Expected) -> &mut Self {
        self.steps.push(Step::Expect(expected));
        self
    }

    pub fn success(&mut self) -> &mut Self {
        self.steps.push(Step::Success);
        self
    }

    pub fn full_node(&mut self) -> &mut Self {
        self.voting_power = 0;
        self
    }

    pub fn is_full_node(&self) -> bool {
        self.voting_power == 0
    }
}

fn unique_id() -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static ID: AtomicUsize = AtomicUsize::new(1);
    ID.fetch_add(1, Ordering::SeqCst)
}

pub struct TestBuilder<S> {
    nodes: Vec<TestNode<S>>,
}

impl<S> Default for TestBuilder<S> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

impl<S> TestBuilder<S>
where
    S: Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self) -> &mut TestNode<S>
    where
        S: Default,
    {
        let node = TestNode::new(self.nodes.len() + 1);
        self.nodes.push(node);
        self.nodes.last_mut().unwrap()
    }

    pub fn build(self) -> Test<S> {
        Test::new(self.nodes)
    }
}

pub struct Test<S> {
    pub id: usize,
    pub nodes: Vec<TestNode<S>>,
    pub private_keys: Vec<PrivateKey>,
    pub validator_set: ValidatorSet,
    pub consensus_base_port: usize,
    pub mempool_base_port: usize,
    pub metrics_base_port: usize,
}

impl<S> Test<S>
where
    S: Send + Sync + 'static,
{
    pub fn new(nodes: Vec<TestNode<S>>) -> Self {
        // Only include nodes with non-zero voting power in the validator set
        let (validators, private_keys) = make_validators(voting_powers(&nodes));
        let validator_set = ValidatorSet::new(validators);
        let id = unique_id();
        let base_port = 20_000 + id * 1000;

        Self {
            id,
            nodes,
            private_keys,
            validator_set,
            consensus_base_port: base_port,
            mempool_base_port: base_port + 100,
            metrics_base_port: base_port + 200,
        }
    }

    pub fn generate_default_configs(&self) -> Vec<Config> {
        (0..self.nodes.len())
            .map(|i| make_node_config(self, i))
            .collect()
    }

    pub fn generate_custom_configs(&self, params: TestParams) -> Vec<Config> {
        let mut configs = self.generate_default_configs();
        for config in &mut configs {
            params.apply_to_config(config);
        }
        configs
    }

    pub async fn run(self, timeout: Duration) {
        let configs = self.generate_default_configs();
        self.run_with_config(configs, timeout).await
    }

    pub async fn run_with_custom_config(self, timeout: Duration, params: TestParams) {
        let configs = self.generate_custom_configs(params);
        self.run_with_config(configs, timeout).await
    }

    pub async fn run_with_config(self, configs: Vec<Config>, timeout: Duration) {
        let _span = error_span!("test", id = %self.id).entered();

        let mut set = JoinSet::new();

        for ((node, config), private_key) in self
            .nodes
            .into_iter()
            .zip(configs.into_iter())
            .zip(self.private_keys.into_iter())
        {
            let validator_set = self.validator_set.clone();

            let home_dir = tempfile::TempDir::with_prefix(format!(
                "informalsystems-malachitebft-starknet-test-{}",
                self.id
            ))
            .unwrap()
            .into_path();

            set.spawn(
                async move {
                    let id = node.id;
                    let result = run_node(node, home_dir, config, validator_set, private_key).await;
                    (id, result)
                }
                .in_current_span(),
            );
        }

        let metrics = tokio::spawn(serve_metrics("127.0.0.1:0".parse().unwrap()));
        let results = tokio::time::timeout(timeout, set.join_all()).await;
        metrics.abort();

        match results {
            Ok(results) => {
                check_results(results);
            }
            Err(_) => {
                error!("Test timed out after {timeout:?}");
                std::process::exit(1);
            }
        }
    }
}

fn check_results(results: Vec<(NodeId, TestResult)>) {
    let mut errors = 0;

    for (id, result) in results {
        let _span = tracing::error_span!("node", %id).entered();
        match result {
            TestResult::Success(reason) => {
                info!("Test succeeded: {reason}");
            }
            TestResult::Failure(reason) => {
                errors += 1;
                error!("Test failed: {reason}");
            }
        }
    }

    if errors > 0 {
        error!("Test failed with {errors} errors");
        std::process::exit(1);
    }
}

pub enum TestResult {
    Success(String),
    Failure(String),
}

#[tracing::instrument("node", skip_all, fields(id = %node.id))]
async fn run_node<S>(
    mut node: TestNode<S>,
    home_dir: PathBuf,
    config: Config,
    validator_set: ValidatorSet,
    private_key: PrivateKey,
) -> TestResult {
    sleep(node.start_delay).await;

    info!("Spawning node with voting power {}", node.voting_power);

    let tx_event = TxEvent::new();
    let mut rx_event = tx_event.subscribe();
    let rx_event_bg = tx_event.subscribe();

    let (mut actor_ref, mut handle) = spawn_node_actor(
        config.clone(),
        home_dir.clone(),
        validator_set.clone(),
        private_key,
        Some(node.start_height),
        tx_event,
        Span::current(),
    )
    .await;

    let decisions = Arc::new(AtomicUsize::new(0));
    let current_height = Arc::new(AtomicUsize::new(0));
    let is_full_node = node.is_full_node();

    let spawn_bg = |mut rx: RxEvent<MockContext>| {
        tokio::spawn({
            let decisions = Arc::clone(&decisions);
            let current_height = Arc::clone(&current_height);

            async move {
                while let Ok(event) = rx.recv().await {
                    match &event {
                        Event::StartedHeight(height) => {
                            current_height.store(height.as_u64() as usize, Ordering::SeqCst);
                        }
                        Event::Decided(_) => {
                            decisions.fetch_add(1, Ordering::SeqCst);
                        }
                        Event::Published(msg) if is_full_node => {
                            panic!("Full nodes unexpectedly publish a consensus message: {msg:?}");
                        }
                        _ => (),
                    }

                    debug!("Event: {event}");
                }
            }
            .in_current_span()
        })
    };

    let mut bg = spawn_bg(rx_event_bg);

    for step in node.steps {
        match step {
            Step::WaitUntil(target_height) => {
                info!("Waiting until node reaches height {target_height}");

                'inner: while let Ok(event) = rx_event.recv().await {
                    let Event::StartedHeight(height) = event else {
                        continue;
                    };

                    info!("Node started height {height}");

                    if height.as_u64() == target_height {
                        break 'inner;
                    }
                }
            }

            Step::Crash(after) => {
                let height = current_height.load(Ordering::SeqCst);

                info!("Node will crash at height {height}");
                sleep(after).await;

                actor_ref.kill_and_wait(None).await.expect("Node must stop");

                bg.abort();
                handle.abort();
            }

            Step::ResetDb => {
                info!("Resetting database");

                let db_path = home_dir.join("db");
                remove_dir_all(&db_path).expect("Database must be removed");
                create_dir_all(&db_path).expect("Database must be created");
            }

            Step::Restart(after) => {
                info!("Node will restart in {after:?}");

                sleep(after).await;

                let tx_event = TxEvent::new();
                let new_rx_event = tx_event.subscribe();
                let new_rx_event_bg = tx_event.subscribe();

                info!("Spawning node");
                let (new_actor_ref, new_handle) = spawn_node_actor(
                    config.clone(),
                    home_dir.clone(),
                    validator_set.clone(),
                    private_key,
                    Some(node.start_height),
                    tx_event,
                    tracing::Span::current(),
                )
                .await;

                info!("Spawned");

                bg = spawn_bg(new_rx_event_bg);

                actor_ref = new_actor_ref;
                handle = new_handle;
                rx_event = new_rx_event;
            }

            Step::OnEvent(on_event) => {
                'inner: while let Ok(event) = rx_event.recv().await {
                    match on_event(event, &mut node.state) {
                        Ok(HandlerResult::WaitForNextEvent) => {
                            continue 'inner;
                        }
                        Ok(HandlerResult::ContinueTest) => {
                            break 'inner;
                        }
                        Err(e) => {
                            actor_ref.stop(Some("Test failed".to_string()));
                            handle.abort();
                            bg.abort();

                            return TestResult::Failure(e.to_string());
                        }
                    }
                }
            }

            Step::Expect(expected) => {
                let actual = decisions.load(Ordering::SeqCst);

                actor_ref.stop(Some("Test is over".to_string()));
                handle.abort();
                bg.abort();

                if expected.check(actual) {
                    return TestResult::Success(format!(
                        "Correct number of decisions: got {actual}, expected: {expected}"
                    ));
                } else {
                    return TestResult::Failure(format!(
                        "Incorrect number of decisions: got {actual}, expected: {expected}"
                    ));
                }
            }

            Step::Success => {
                break;
            }

            Step::Fail(reason) => {
                actor_ref.stop(Some("Test failed".to_string()));
                handle.abort();
                bg.abort();

                return TestResult::Failure(reason);
            }
        }
    }

    return TestResult::Success("OK".to_string());
}

pub fn init_logging(test_module: &str) {
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    let debug_vars = &[("ACTIONS_RUNNER_DEBUG", "true"), ("MALACHITE_DEBUG", "1")];
    let enable_debug = debug_vars
        .iter()
        .any(|(k, v)| std::env::var(k).as_deref() == Ok(v));

    let directive = if enable_debug {
        format!("informalsystems=info,{test_module}=debug,ractor=error,debug")
    } else {
        format!("informalsystems=debug,{test_module}=debug,ractor=error,warn")
    };

    let filter = EnvFilter::builder().parse(directive).unwrap();

    pub fn enable_ansi() -> bool {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
    }

    // Construct a tracing subscriber with the supplied filter and enable reloading.
    let builder = FmtSubscriber::builder()
        .with_target(false)
        .with_env_filter(filter)
        .with_test_writer()
        .with_ansi(enable_ansi())
        .with_thread_ids(false);

    let subscriber = builder.finish();

    if let Err(e) = subscriber.try_init() {
        eprintln!("Failed to initialize logging: {e}");
    }
}

use bytesize::ByteSize;

use malachitebft_config::{
    ConsensusConfig, MempoolConfig, MetricsConfig, P2pConfig, RuntimeConfig, TimeoutConfig,
    ValuePayload,
};

fn transport_from_env(default: TransportProtocol) -> TransportProtocol {
    if let Ok(protocol) = std::env::var("MALACHITE_TRANSPORT") {
        TransportProtocol::from_str(&protocol).unwrap_or(default)
    } else {
        default
    }
}

pub fn make_node_config<S>(test: &Test<S>, i: usize) -> NodeConfig {
    let transport = transport_from_env(TransportProtocol::Tcp);
    let protocol = PubSubProtocol::default();

    NodeConfig {
        moniker: format!("node-{}", test.nodes[i].id),
        logging: LoggingConfig::default(),
        consensus: ConsensusConfig {
            max_block_size: ByteSize::mib(1),
            value_payload: ValuePayload::default(),
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                transport,
                protocol,
                discovery: DiscoveryConfig::default(),
                listen_addr: transport.multiaddr("127.0.0.1", test.consensus_base_port + i),
                persistent_peers: (0..test.nodes.len())
                    .filter(|j| i != *j)
                    .map(|j| transport.multiaddr("127.0.0.1", test.consensus_base_port + j))
                    .collect(),
                ..Default::default()
            },
        },
        mempool: MempoolConfig {
            p2p: P2pConfig {
                transport,
                protocol,
                listen_addr: transport.multiaddr("127.0.0.1", test.mempool_base_port + i),
                persistent_peers: (0..test.nodes.len())
                    .filter(|j| i != *j)
                    .map(|j| transport.multiaddr("127.0.0.1", test.mempool_base_port + j))
                    .collect(),
                ..Default::default()
            },
            max_tx_count: 10000,
            gossip_batch_size: 100,
        },
        sync: SyncConfig {
            enabled: true,
            status_update_interval: Duration::from_secs(2),
            request_timeout: Duration::from_secs(5),
        },
        metrics: MetricsConfig {
            enabled: false,
            listen_addr: format!("127.0.0.1:{}", test.metrics_base_port + i)
                .parse()
                .unwrap(),
        },
        runtime: RuntimeConfig::single_threaded(),
        test: TestConfig::default(),
    }
}

fn voting_powers<S>(nodes: &[TestNode<S>]) -> Vec<VotingPower> {
    nodes.iter().map(|node| node.voting_power).collect()
}

pub fn make_validators(voting_powers: Vec<VotingPower>) -> (Vec<Validator>, Vec<PrivateKey>) {
    let mut rng = StdRng::seed_from_u64(0x42);

    let mut validators = Vec::with_capacity(voting_powers.len());
    let mut private_keys = Vec::with_capacity(voting_powers.len());

    for vp in voting_powers {
        let sk = PrivateKey::generate(&mut rng);
        let val = Validator::new(sk.public_key(), vp);

        private_keys.push(sk);

        if vp > 0 {
            validators.push(val);
        }
    }

    (validators, private_keys)
}

use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;

#[tracing::instrument(name = "metrics", skip_all)]
async fn serve_metrics(listen_addr: SocketAddr) {
    let app = Router::new().route("/metrics", get(get_metrics));
    let listener = TcpListener::bind(listen_addr).await.unwrap();
    let address = listener.local_addr().unwrap();

    async fn get_metrics() -> String {
        let mut buf = String::new();
        malachitebft_metrics::export(&mut buf);
        buf
    }

    info!(%address, "Serving metrics");
    axum::serve(listener, app).await.unwrap();
}
