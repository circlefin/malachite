use core::fmt;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use bytesize::ByteSize;
use config as config_rs;
use malachitebft_core_types::TimeoutKind;
use multiaddr::Multiaddr;
use serde::{Deserialize, Serialize};

/// Malachite configuration options
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// A custom human-readable name for this node
    pub moniker: String,

    /// Log configuration options
    pub logging: LoggingConfig,

    /// Consensus configuration options
    pub consensus: ConsensusConfig,

    /// Mempool configuration options
    pub mempool: MempoolConfig,

    /// Sync configuration options
    pub sync: SyncConfig,

    /// Metrics configuration options
    pub metrics: MetricsConfig,

    /// Runtime configuration options
    pub runtime: RuntimeConfig,

    /// Test configuration
    #[serde(default)]
    pub test: TestConfig,
}

/// load_config parses the environment variables and loads the provided config file path
/// to create a Config struct.
pub fn load_config(config_file_path: &Path, prefix: Option<&str>) -> Result<Config, String> {
    config_rs::Config::builder()
        .add_source(config::File::from(config_file_path))
        .add_source(config::Environment::with_prefix(prefix.unwrap_or("MALACHITE")).separator("__"))
        .build()
        .map_err(|error| error.to_string())?
        .try_deserialize()
        .map_err(|error| error.to_string())
}

/// P2P configuration options
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct P2pConfig {
    /// Address to listen for incoming connections
    pub listen_addr: Multiaddr,

    /// List of nodes to keep persistent connections to
    pub persistent_peers: Vec<Multiaddr>,

    /// Peer discovery
    #[serde(default)]
    pub discovery: DiscoveryConfig,

    /// Transport protocol to use
    pub transport: TransportProtocol,

    /// The type of pub-sub protocol to use for consensus
    pub protocol: PubSubProtocol,

    /// The maximum size of messages to send over pub-sub
    pub pubsub_max_size: ByteSize,

    /// The maximum size of messages to send over RPC
    pub rpc_max_size: ByteSize,
}

impl Default for P2pConfig {
    fn default() -> Self {
        P2pConfig {
            listen_addr: Multiaddr::empty(),
            persistent_peers: vec![],
            discovery: Default::default(),
            transport: Default::default(),
            protocol: Default::default(),
            rpc_max_size: ByteSize::mib(10),
            pubsub_max_size: ByteSize::mib(4),
        }
    }
}
/// Peer Discovery configuration options
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct DiscoveryConfig {
    /// Enable peer discovery
    #[serde(default)]
    pub enabled: bool,

    /// Bootstrap protocol
    #[serde(default)]
    pub bootstrap_protocol: BootstrapProtocol,

    /// Selector
    #[serde(default)]
    pub selector: Selector,

    /// Number of outbound peers
    #[serde(default)]
    pub num_outbound_peers: usize,

    /// Number of inbound peers
    #[serde(default)]
    pub num_inbound_peers: usize,

    /// Ephemeral connection timeout
    #[serde(default)]
    pub ephemeral_connection_timeout: Duration,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BootstrapProtocol {
    #[default]
    Kademlia,
    Full,
}

impl BootstrapProtocol {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Kademlia => "kademlia",
            Self::Full => "full",
        }
    }
}

impl FromStr for BootstrapProtocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "kademlia" => Ok(Self::Kademlia),
            "full" => Ok(Self::Full),
            e => Err(format!(
                "unknown bootstrap protocol: {e}, available: kademlia, full"
            )),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Selector {
    #[default]
    Kademlia,
    Random,
}

impl Selector {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Kademlia => "kademlia",
            Self::Random => "random",
        }
    }
}

impl FromStr for Selector {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "kademlia" => Ok(Self::Kademlia),
            "random" => Ok(Self::Random),
            e => Err(format!(
                "unknown selector: {e}, available: kademlia, random"
            )),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportProtocol {
    #[default]
    Tcp,
    Quic,
}

impl TransportProtocol {
    pub fn multiaddr(&self, host: &str, port: usize) -> Multiaddr {
        match self {
            Self::Tcp => format!("/ip4/{host}/tcp/{port}").parse().unwrap(),
            Self::Quic => format!("/ip4/{host}/udp/{port}/quic-v1").parse().unwrap(),
        }
    }
}

impl FromStr for TransportProtocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tcp" => Ok(Self::Tcp),
            "quic" => Ok(Self::Quic),
            e => Err(format!(
                "unknown transport protocol: {e}, available: tcp, quic"
            )),
        }
    }
}

/// The type of pub-sub protocol.
/// If multiple protocols are configured in the configuration file, the first one from this list
/// will be used.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PubSubProtocol {
    GossipSub(GossipSubConfig),
    Broadcast,
}

impl Default for PubSubProtocol {
    fn default() -> Self {
        Self::GossipSub(GossipSubConfig::default())
    }
}

/// GossipSub configuration
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "gossipsub::RawConfig", default)]
pub struct GossipSubConfig {
    /// Target number of peers for the mesh network (D in the GossipSub spec)
    mesh_n: usize,

    /// Maximum number of peers in mesh network before removing some (D_high in the GossipSub spec)
    mesh_n_high: usize,

    /// Minimum number of peers in mesh network before adding more (D_low in the spec)
    mesh_n_low: usize,

    /// Minimum number of outbound peers in the mesh network before adding more (D_out in the spec).
    /// This value must be smaller or equal than `mesh_n / 2` and smaller than `mesh_n_low`.
    /// When this value is set to 0 or does not meet the above constraints,
    /// it will be calculated as `max(1, min(mesh_n / 2, mesh_n_low - 1))`
    mesh_outbound_min: usize,
}

impl Default for GossipSubConfig {
    fn default() -> Self {
        Self::new(6, 12, 4, 2)
    }
}

impl GossipSubConfig {
    /// Create a new, valid GossipSub configuration.
    pub fn new(
        mesh_n: usize,
        mesh_n_high: usize,
        mesh_n_low: usize,
        mesh_outbound_min: usize,
    ) -> Self {
        let mut result = Self {
            mesh_n,
            mesh_n_high,
            mesh_n_low,
            mesh_outbound_min,
        };

        result.adjust();
        result
    }

    /// Adjust the configuration values.
    pub fn adjust(&mut self) {
        use std::cmp::{max, min};

        if self.mesh_n == 0 {
            self.mesh_n = 6;
        }

        if self.mesh_n_high == 0 || self.mesh_n_high < self.mesh_n {
            self.mesh_n_high = self.mesh_n * 2;
        }

        if self.mesh_n_low == 0 || self.mesh_n_low > self.mesh_n {
            self.mesh_n_low = self.mesh_n * 2 / 3;
        }

        if self.mesh_outbound_min == 0
            || self.mesh_outbound_min > self.mesh_n / 2
            || self.mesh_outbound_min >= self.mesh_n_low
        {
            self.mesh_outbound_min = max(1, min(self.mesh_n / 2, self.mesh_n_low - 1));
        }
    }

    pub fn mesh_n(&self) -> usize {
        self.mesh_n
    }

    pub fn mesh_n_high(&self) -> usize {
        self.mesh_n_high
    }

    pub fn mesh_n_low(&self) -> usize {
        self.mesh_n_low
    }

    pub fn mesh_outbound_min(&self) -> usize {
        self.mesh_outbound_min
    }
}

mod gossipsub {
    #[derive(serde::Deserialize)]
    pub struct RawConfig {
        #[serde(default)]
        mesh_n: usize,
        #[serde(default)]
        mesh_n_high: usize,
        #[serde(default)]
        mesh_n_low: usize,
        #[serde(default)]
        mesh_outbound_min: usize,
    }

    impl From<RawConfig> for super::GossipSubConfig {
        fn from(raw: RawConfig) -> Self {
            super::GossipSubConfig::new(
                raw.mesh_n,
                raw.mesh_n_high,
                raw.mesh_n_low,
                raw.mesh_outbound_min,
            )
        }
    }
}

/// Mempool configuration options
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MempoolConfig {
    /// P2P configuration options
    pub p2p: P2pConfig,

    /// Maximum number of transactions
    pub max_tx_count: usize,

    /// Maximum number of transactions to gossip at once in a batch
    pub gossip_batch_size: usize,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Enable Sync
    pub enabled: bool,

    /// Interval at which to update other peers of our status
    #[serde(with = "humantime_serde")]
    pub status_update_interval: Duration,

    /// Timeout duration for block sync requests
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            status_update_interval: Duration::from_secs(10),
            request_timeout: Duration::from_secs(10),
        }
    }
}

/// Consensus configuration options
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Max block size
    pub max_block_size: ByteSize,

    /// Timeouts
    #[serde(flatten)]
    pub timeouts: TimeoutConfig,

    /// Message types that can carry values
    pub value_payload: ValuePayload,

    /// P2P configuration options
    pub p2p: P2pConfig,
}

/// Message types required by consensus to deliver the value being proposed
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValuePayload {
    #[default]
    PartsOnly,
    ProposalOnly, // TODO - add small block app to test this option
    ProposalAndParts,
}

/// Timeouts
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// How long we wait for a proposal block before prevoting nil
    #[serde(with = "humantime_serde")]
    pub timeout_propose: Duration,

    /// How much timeout_propose increases with each round
    #[serde(with = "humantime_serde")]
    pub timeout_propose_delta: Duration,

    /// How long we wait after receiving +2/3 prevotes for “anything” (ie. not a single block or nil)
    #[serde(with = "humantime_serde")]
    pub timeout_prevote: Duration,

    /// How much the timeout_prevote increases with each round
    #[serde(with = "humantime_serde")]
    pub timeout_prevote_delta: Duration,

    /// How long we wait after receiving +2/3 precommits for “anything” (ie. not a single block or nil)
    #[serde(with = "humantime_serde")]
    pub timeout_precommit: Duration,

    /// How much the timeout_precommit increases with each round
    #[serde(with = "humantime_serde")]
    pub timeout_precommit_delta: Duration,

    /// How long we wait after committing a block, before starting on the new
    /// height (this gives us a chance to receive some more precommits, even
    /// though we already have +2/3).
    #[serde(with = "humantime_serde")]
    pub timeout_commit: Duration,

    /// How long we stay in preovte or precommit steps before starting
    /// the vote synchronization protocol.
    #[serde(with = "humantime_serde")]
    pub timeout_step: Duration,
}

impl TimeoutConfig {
    pub fn timeout_duration(&self, step: TimeoutKind) -> Duration {
        match step {
            TimeoutKind::Propose => self.timeout_propose,
            TimeoutKind::Prevote => self.timeout_prevote,
            TimeoutKind::Precommit => self.timeout_precommit,
            TimeoutKind::Commit => self.timeout_commit,
            TimeoutKind::PrevoteTimeLimit => self.timeout_step,
            TimeoutKind::PrecommitTimeLimit => self.timeout_step,
        }
    }

    pub fn delta_duration(&self, step: TimeoutKind) -> Option<Duration> {
        match step {
            TimeoutKind::Propose => Some(self.timeout_propose_delta),
            TimeoutKind::Prevote => Some(self.timeout_prevote_delta),
            TimeoutKind::Precommit => Some(self.timeout_precommit_delta),
            TimeoutKind::Commit => None,
            TimeoutKind::PrevoteTimeLimit => None,
            TimeoutKind::PrecommitTimeLimit => None,
        }
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            timeout_propose: Duration::from_secs(3),
            timeout_propose_delta: Duration::from_millis(500),
            timeout_prevote: Duration::from_secs(1),
            timeout_prevote_delta: Duration::from_millis(500),
            timeout_precommit: Duration::from_secs(1),
            timeout_precommit_delta: Duration::from_millis(500),
            timeout_commit: Duration::from_secs(0),
            timeout_step: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable the metrics server
    pub enabled: bool,

    /// Address at which to serve the metrics at
    pub listen_addr: SocketAddr,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        MetricsConfig {
            enabled: false,
            listen_addr: SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 9000),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "flavor", rename_all = "snake_case")]
pub enum RuntimeConfig {
    /// Single-threaded runtime
    #[default]
    SingleThreaded,

    /// Multi-threaded runtime
    MultiThreaded {
        /// Number of worker threads
        worker_threads: usize,
    },
}

impl RuntimeConfig {
    pub fn single_threaded() -> Self {
        Self::SingleThreaded
    }

    pub fn multi_threaded(worker_threads: usize) -> Self {
        Self::MultiThreaded { worker_threads }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VoteExtensionsConfig {
    pub enabled: bool,
    pub size: ByteSize,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestConfig {
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub time_allowance_factor: f32,
    #[serde(with = "humantime_serde")]
    pub exec_time_per_tx: Duration,
    pub max_retain_blocks: usize,
    #[serde(default)]
    pub vote_extensions: VoteExtensionsConfig,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            tx_size: ByteSize::kib(1),
            txs_per_part: 256,
            time_allowance_factor: 0.5,
            exec_time_per_tx: Duration::from_millis(1),
            max_retain_blocks: 1000,
            vote_extensions: VoteExtensionsConfig::default(),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub log_level: LogLevel,
    pub log_format: LogFormat,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    #[default]
    Debug,
    Warn,
    Info,
    Error,
}

impl FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "warn" => Ok(LogLevel::Warn),
            "info" => Ok(LogLevel::Info),
            "error" => Ok(LogLevel::Error),
            e => Err(format!("Invalid log level: {e}")),
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Plaintext,
    Json,
}

impl FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "plaintext" => Ok(LogFormat::Plaintext),
            "json" => Ok(LogFormat::Json),
            e => Err(format!("Invalid log format: {e}")),
        }
    }
}

impl fmt::Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogFormat::Plaintext => write!(f, "plaintext"),
            LogFormat::Json => write!(f, "json"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_config_file() {
        let file = include_str!("../../../examples/channel/config.toml");
        let config = toml::from_str::<Config>(file).unwrap();
        assert_eq!(config.consensus.timeouts, TimeoutConfig::default());
        assert_eq!(config.test, TestConfig::default());

        let tmp_file = std::env::temp_dir().join("informalsystems-malachitebft-config.toml");
        std::fs::write(&tmp_file, file).unwrap();

        let config = load_config(&tmp_file, None).unwrap();
        assert_eq!(config.consensus.timeouts, TimeoutConfig::default());
        assert_eq!(config.test, TestConfig::default());

        std::fs::remove_file(tmp_file).unwrap();
    }

    #[test]
    fn log_format() {
        assert_eq!(
            LogFormat::from_str("yaml"),
            Err("Invalid log format: yaml".to_string())
        )
    }

    #[test]
    fn timeout_durations() {
        let t = TimeoutConfig::default();
        assert_eq!(t.timeout_duration(TimeoutKind::Propose), t.timeout_propose);
        assert_eq!(t.timeout_duration(TimeoutKind::Prevote), t.timeout_prevote);
        assert_eq!(
            t.timeout_duration(TimeoutKind::Precommit),
            t.timeout_precommit
        );
        assert_eq!(t.timeout_duration(TimeoutKind::Commit), t.timeout_commit);
    }

    #[test]
    fn runtime_multi_threaded() {
        assert_eq!(
            RuntimeConfig::multi_threaded(5),
            RuntimeConfig::MultiThreaded { worker_threads: 5 }
        );
    }

    #[test]
    fn log_formatting() {
        assert_eq!(
            format!(
                "{} {} {} {} {}",
                LogLevel::Trace,
                LogLevel::Debug,
                LogLevel::Warn,
                LogLevel::Info,
                LogLevel::Error
            ),
            "trace debug warn info error"
        );

        assert_eq!(
            format!("{} {}", LogFormat::Plaintext, LogFormat::Json),
            "plaintext json"
        );
    }
}
