use core::fmt;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use bytesize::ByteSize;
use malachite_common::TimeoutStep;
use multiaddr::Multiaddr;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum App {
    #[default]
    #[serde(rename = "starknet")]
    Starknet,
}

impl fmt::Display for App {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starknet => write!(f, "starknet"),
        }
    }
}

impl FromStr for App {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "starknet" => Ok(Self::Starknet),
            _ => Err(format!("unknown application: {s}, available: starknet")),
        }
    }
}

/// Malachite configuration options
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// A custom human-readable name for this node
    pub moniker: String,

    /// The name of the application to run
    pub app: App,

    /// Log configuration options
    pub logging: LoggingConfig,

    /// Consensus configuration options
    pub consensus: ConsensusConfig,

    /// Mempool configuration options
    pub mempool: MempoolConfig,

    /// Metrics configuration options
    pub metrics: MetricsConfig,

    /// Runtime configuration options
    pub runtime: RuntimeConfig,

    /// Test configuration
    #[serde(default)]
    pub test: TestConfig,
}

/// P2P configuration options
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct P2pConfig {
    // Address to listen for incoming connections
    pub listen_addr: Multiaddr,

    /// List of nodes to keep persistent connections to
    pub persistent_peers: Vec<Multiaddr>,

    /// Transport protocol to use
    pub transport: TransportProtocol,

    /// The type of pub-sub protocol to use for consensus
    pub protocol: PubSubProtocol,
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

/// The type of pub-sub protocol
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PubSubProtocol {
    #[default]
    GossipSub,
    Broadcast,
}

/// Mempool configuration options
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MempoolConfig {
    /// P2P configuration options
    pub p2p: P2pConfig,

    /// Maximum number of transactions
    pub max_tx_count: usize,

    /// Maximum number of transactions to gossip at once in a batch
    pub gossip_batch_size: usize,
}

/// Consensus configuration options
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Max block size
    pub max_block_size: ByteSize,

    /// Timeouts
    #[serde(flatten)]
    pub timeouts: TimeoutConfig,

    /// P2P configuration options
    pub p2p: P2pConfig,
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
}

impl TimeoutConfig {
    pub fn timeout_duration(&self, step: TimeoutStep) -> Duration {
        match step {
            TimeoutStep::Propose => self.timeout_propose,
            TimeoutStep::Prevote => self.timeout_prevote,
            TimeoutStep::Precommit => self.timeout_precommit,
            TimeoutStep::Commit => self.timeout_commit,
        }
    }

    pub fn delta_duration(&self, step: TimeoutStep) -> Option<Duration> {
        match step {
            TimeoutStep::Propose => Some(self.timeout_propose_delta),
            TimeoutStep::Prevote => Some(self.timeout_prevote_delta),
            TimeoutStep::Precommit => Some(self.timeout_precommit_delta),
            TimeoutStep::Commit => None,
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

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestConfig {
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub time_allowance_factor: f32,
    #[serde(with = "humantime_serde")]
    pub exec_time_per_tx: Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            tx_size: ByteSize::kib(1),
            txs_per_part: 256,
            time_allowance_factor: 0.5,
            exec_time_per_tx: Duration::from_millis(1),
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
        let file = include_str!("../../../config.toml");
        let config = toml::from_str::<Config>(file).unwrap();
        assert_eq!(config.consensus.timeouts, TimeoutConfig::default());
        assert_eq!(config.test, TestConfig::default());
    }

    #[test]
    fn log_format() {
        assert_eq!(
            LogFormat::from_str("yaml"),
            Err("Invalid log format: yaml".to_string())
        )
    }

    #[test]
    fn parse_invalid_app() {
        assert_eq!(
            App::from_str("invalid"),
            Err("unknown application: invalid, available: starknet".to_string())
        );
    }

    #[test]
    fn timeout_durations() {
        let t = TimeoutConfig::default();
        assert_eq!(t.timeout_duration(TimeoutStep::Propose), t.timeout_propose);
        assert_eq!(t.timeout_duration(TimeoutStep::Prevote), t.timeout_prevote);
        assert_eq!(
            t.timeout_duration(TimeoutStep::Precommit),
            t.timeout_precommit
        );
        assert_eq!(t.timeout_duration(TimeoutStep::Commit), t.timeout_commit);
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
