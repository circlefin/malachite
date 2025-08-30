use core::fmt;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;

use bytesize::ByteSize;
use malachitebft_core_types::TimeoutKind;
use multiaddr::Multiaddr;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtocolNames {
    pub consensus: String,

    pub discovery_kad: String,

    pub discovery_regres: String,

    pub sync: String,
}

impl Default for ProtocolNames {
    fn default() -> Self {
        Self {
            consensus: "/malachitebft-core-consensus/v1beta1".to_string(),
            discovery_kad: "/malachitebft-discovery/kad/v1beta1".to_string(),
            discovery_regres: "/malachitebft-discovery/reqres/v1beta1".to_string(),
            sync: "/malachitebft-sync/v1beta1".to_string(),
        }
    }
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

    /// The type of pub-sub protocol to use for consensus
    pub protocol: PubSubProtocol,

    /// The maximum size of messages to send over pub-sub
    pub pubsub_max_size: ByteSize,

    /// The maximum size of messages to send over RPC
    pub rpc_max_size: ByteSize,

    /// Protocol name configuration
    #[serde(default)]
    pub protocol_names: ProtocolNames,
}

impl Default for P2pConfig {
    fn default() -> Self {
        P2pConfig {
            listen_addr: Multiaddr::empty(),
            persistent_peers: vec![],
            discovery: Default::default(),
            protocol: Default::default(),
            rpc_max_size: ByteSize::mib(10),
            pubsub_max_size: ByteSize::mib(4),
            protocol_names: Default::default(),
        }
    }
}

/// Peer Discovery configuration options
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
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

    /// Maximum number of connections per peer
    #[serde(default)]
    pub max_connections_per_peer: usize,

    /// Ephemeral connection timeout
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    pub ephemeral_connection_timeout: Duration,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        DiscoveryConfig {
            enabled: false,
            bootstrap_protocol: Default::default(),
            selector: Default::default(),
            num_outbound_peers: 0,
            num_inbound_peers: 20,
            max_connections_per_peer: 5,
            ephemeral_connection_timeout: Default::default(),
        }
    }
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "load_type", rename_all = "snake_case")]
pub enum MempoolLoadType {
    NoLoad,
    UniformLoad(mempool_load::UniformLoadConfig),
    NonUniformLoad(mempool_load::NonUniformLoadConfig),
}

impl Default for MempoolLoadType {
    fn default() -> Self {
        Self::NoLoad
    }
}

pub mod mempool_load {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct NonUniformLoadConfig {
        /// Base transaction count
        pub base_count: i32,

        /// Base transaction size
        pub base_size: i32,

        /// How much the transaction count can vary
        pub count_variation: std::ops::Range<i32>,

        /// How much the transaction size can vary
        pub size_variation: std::ops::Range<i32>,

        /// Chance of generating a spike.
        /// e.g. 0.1 = 10% chance of spike
        pub spike_probability: f64,

        /// Multiplier for spike transactions
        /// e.g. 10 = 10x more transactions during spike
        pub spike_multiplier: usize,

        /// Range of intervals between generating load, in milliseconds
        pub sleep_interval: std::ops::Range<u64>,
    }

    impl Default for NonUniformLoadConfig {
        fn default() -> Self {
            Self {
                base_count: 100,
                base_size: 256,
                count_variation: -100..200,
                size_variation: -64..128,
                spike_probability: 0.10,
                spike_multiplier: 2,
                sleep_interval: 1000..5000,
            }
        }
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct UniformLoadConfig {
        /// Interval at which to generate load
        #[serde(with = "humantime_serde")]
        pub interval: Duration,

        /// Number of transactions to generate
        pub count: usize,

        /// Size of each generated transaction
        pub size: ByteSize,
    }

    impl Default for UniformLoadConfig {
        fn default() -> Self {
            Self {
                interval: Duration::from_secs(1),
                count: 1000,
                size: ByteSize::b(256),
            }
        }
    }
}

/// Mempool configuration options
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MempoolLoadConfig {
    /// Mempool loading type
    #[serde(flatten)]
    pub load_type: MempoolLoadType,
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

    /// Mempool load configuration options
    pub load: MempoolLoadConfig,
}

/// ValueSync configuration options
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValueSyncConfig {
    /// Enable ValueSync
    pub enabled: bool,

    /// Interval at which to update other peers of our status
    #[serde(with = "humantime_serde")]
    pub status_update_interval: Duration,

    /// Timeout duration for sync requests
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,

    /// Maximum size of a request
    pub max_request_size: ByteSize,

    /// Maximum size of a response
    pub max_response_size: ByteSize,

    /// Maximum number of parallel requests to send
    pub parallel_requests: usize,

    /// Scoring strategy for peers
    #[serde(default)]
    pub scoring_strategy: ScoringStrategy,

    /// Threshold for considering a peer inactive
    #[serde(with = "humantime_serde")]
    pub inactive_threshold: Duration,

    /// Maximum number of decided values to request in a single batch
    pub batch_size: usize,
}

impl Default for ValueSyncConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            status_update_interval: Duration::from_secs(10),
            request_timeout: Duration::from_secs(10),
            max_request_size: ByteSize::mib(1),
            max_response_size: ByteSize::mib(10),
            parallel_requests: 5,
            scoring_strategy: ScoringStrategy::default(),
            inactive_threshold: Duration::from_secs(60),
            batch_size: 5,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoringStrategy {
    #[default]
    Ema,
}

impl ScoringStrategy {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ema => "ema",
        }
    }
}

impl FromStr for ScoringStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ema" => Ok(Self::Ema),
            e => Err(format!("unknown scoring strategy: {e}, available: ema")),
        }
    }
}

/// Consensus configuration options
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Timeouts
    #[serde(flatten)]
    pub timeouts: TimeoutConfig,

    /// P2P configuration options
    pub p2p: P2pConfig,

    /// Message types that can carry values
    pub value_payload: ValuePayload,

    /// Size of the consensus input queue
    ///
    /// # Deprecated
    /// This setting is deprecated and will be removed in the future.
    /// The queue capacity is now derived from the `sync.parallel_requests` setting.
    #[serde(default)]
    pub queue_capacity: usize,
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

impl ValuePayload {
    pub fn include_parts(&self) -> bool {
        match self {
            Self::ProposalOnly => false,
            Self::PartsOnly | Self::ProposalAndParts => true,
        }
    }

    pub fn include_proposal(&self) -> bool {
        match self {
            Self::PartsOnly => false,
            Self::ProposalOnly | Self::ProposalAndParts => true,
        }
    }
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

    /// How long we wait after entering a round before starting
    /// the rebroadcast liveness protocol
    #[serde(with = "humantime_serde")]
    pub timeout_rebroadcast: Duration,
}

impl TimeoutConfig {
    pub fn timeout_duration(&self, step: TimeoutKind) -> Duration {
        match step {
            TimeoutKind::Propose => self.timeout_propose,
            TimeoutKind::Prevote => self.timeout_prevote,
            TimeoutKind::Precommit => self.timeout_precommit,
            TimeoutKind::Rebroadcast => {
                self.timeout_propose + self.timeout_prevote + self.timeout_precommit
            }
        }
    }

    pub fn delta_duration(&self, step: TimeoutKind) -> Option<Duration> {
        match step {
            TimeoutKind::Propose => Some(self.timeout_propose_delta),
            TimeoutKind::Prevote => Some(self.timeout_prevote_delta),
            TimeoutKind::Precommit => Some(self.timeout_precommit_delta),
            TimeoutKind::Rebroadcast => None,
        }
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        let timeout_propose = Duration::from_secs(3);
        let timeout_prevote = Duration::from_secs(1);
        let timeout_precommit = Duration::from_secs(1);
        let timeout_rebroadcast = timeout_propose + timeout_prevote + timeout_precommit;

        Self {
            timeout_propose,
            timeout_propose_delta: Duration::from_millis(500),
            timeout_prevote,
            timeout_prevote_delta: Duration::from_millis(500),
            timeout_precommit,
            timeout_precommit_delta: Duration::from_millis(500),
            timeout_rebroadcast,
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
    pub max_block_size: ByteSize,
    pub txs_per_part: usize,
    pub time_allowance_factor: f32,
    #[serde(with = "humantime_serde")]
    pub exec_time_per_tx: Duration,
    pub max_retain_blocks: usize,
    #[serde(default)]
    pub vote_extensions: VoteExtensionsConfig,
    #[serde(default)]
    pub stable_block_times: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            max_block_size: ByteSize::mib(1),
            txs_per_part: 256,
            time_allowance_factor: 0.5,
            exec_time_per_tx: Duration::from_millis(1),
            max_retain_blocks: 1000,
            vote_extensions: VoteExtensionsConfig::default(),
            stable_block_times: false,
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

    #[test]
    fn protocol_names_default() {
        let protocol_names = ProtocolNames::default();
        assert_eq!(
            protocol_names.consensus,
            "/malachitebft-core-consensus/v1beta1"
        );
        assert_eq!(
            protocol_names.discovery_kad,
            "/malachitebft-discovery/kad/v1beta1"
        );
        assert_eq!(
            protocol_names.discovery_regres,
            "/malachitebft-discovery/reqres/v1beta1"
        );
        assert_eq!(protocol_names.sync, "/malachitebft-sync/v1beta1");
    }

    #[test]
    fn protocol_names_serde() {
        use serde_json;

        // Test serialization
        let protocol_names = ProtocolNames {
            consensus: "/custom-consensus/v1".to_string(),
            discovery_kad: "/custom-discovery/kad/v1".to_string(),
            discovery_regres: "/custom-discovery/reqres/v1".to_string(),
            sync: "/custom-sync/v1".to_string(),
        };

        let json = serde_json::to_string(&protocol_names).unwrap();

        // Test deserialization
        let deserialized: ProtocolNames = serde_json::from_str(&json).unwrap();
        assert_eq!(protocol_names, deserialized);
    }

    #[test]
    fn p2p_config_with_protocol_names() {
        let config = P2pConfig::default();

        // Verify protocol_names field exists and has defaults
        assert_eq!(config.protocol_names, ProtocolNames::default());

        // Test with custom protocol names
        let custom_protocol_names = ProtocolNames {
            consensus: "/test-network/consensus/v1".to_string(),
            discovery_kad: "/test-network/discovery/kad/v1".to_string(),
            discovery_regres: "/test-network/discovery/reqres/v1".to_string(),
            sync: "/test-network/sync/v1".to_string(),
        };

        let config_with_custom = P2pConfig {
            protocol_names: custom_protocol_names.clone(),
            ..Default::default()
        };

        assert_eq!(config_with_custom.protocol_names, custom_protocol_names);
    }

    #[test]
    fn protocol_names_toml_deserialization() {
        let toml_content = r#"
        timeout_propose = "3s"
        timeout_propose_delta = "500ms"
        timeout_prevote = "1s"
        timeout_prevote_delta = "500ms"
        timeout_precommit = "1s"
        timeout_precommit_delta = "500ms"
        timeout_rebroadcast = "5s"
        value_payload = "parts-only"
        
        [p2p]
        listen_addr = "/ip4/0.0.0.0/tcp/0"
        persistent_peers = []
        pubsub_max_size = "4 MiB"
        rpc_max_size = "10 MiB"
        
        [p2p.protocol_names]
        consensus = "/custom-network/consensus/v2"
        discovery_kad = "/custom-network/discovery/kad/v2"
        discovery_regres = "/custom-network/discovery/reqres/v2"
        sync = "/custom-network/sync/v2"
        
        [p2p.protocol]
        type = "gossipsub"
        "#;

        let config: ConsensusConfig = toml::from_str(toml_content).unwrap();

        assert_eq!(
            config.p2p.protocol_names.consensus,
            "/custom-network/consensus/v2"
        );
        assert_eq!(
            config.p2p.protocol_names.discovery_kad,
            "/custom-network/discovery/kad/v2"
        );
        assert_eq!(
            config.p2p.protocol_names.discovery_regres,
            "/custom-network/discovery/reqres/v2"
        );
        assert_eq!(config.p2p.protocol_names.sync, "/custom-network/sync/v2");
    }

    #[test]
    fn protocol_names_toml_defaults_when_missing() {
        let toml_content = r#"
        timeout_propose = "3s"
        timeout_propose_delta = "500ms"
        timeout_prevote = "1s"
        timeout_prevote_delta = "500ms"
        timeout_precommit = "1s"
        timeout_precommit_delta = "500ms"
        timeout_rebroadcast = "5s"
        value_payload = "parts-only"
        
        [p2p]
        listen_addr = "/ip4/0.0.0.0/tcp/0"
        persistent_peers = []
        pubsub_max_size = "4 MiB"
        rpc_max_size = "10 MiB"
        
        [p2p.protocol]
        type = "gossipsub"
        "#;

        let config: ConsensusConfig = toml::from_str(toml_content).unwrap();

        // Should use defaults when protocol_names section is missing
        assert_eq!(config.p2p.protocol_names, ProtocolNames::default());
    }
}
