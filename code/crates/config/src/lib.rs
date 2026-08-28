use core::fmt;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;

use bytesize::ByteSize;
use multiaddr::Multiaddr;
use serde::{Deserialize, Serialize};

mod utils;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtocolNames {
    pub consensus: String,

    pub discovery_kad: String,

    pub discovery_regres: String,

    pub sync: String,

    pub validator_proof: String,
}

impl Default for ProtocolNames {
    fn default() -> Self {
        Self {
            consensus: "/malachitebft-core-consensus/v1beta1".to_string(),
            discovery_kad: "/malachitebft-discovery/kad/v1beta1".to_string(),
            discovery_regres: "/malachitebft-discovery/reqres/v1beta1".to_string(),
            sync: "/malachitebft-sync/v1beta1".to_string(),
            validator_proof: "/malachitebft-validator-proof/v1".to_string(),
        }
    }
}

/// GossipSub topic / broadcast channel names.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelNames {
    pub consensus: String,

    pub proposal_parts: String,

    pub sync: String,

    pub liveness: String,
}

impl Default for ChannelNames {
    fn default() -> Self {
        Self {
            consensus: "/consensus".to_string(),
            proposal_parts: "/proposal_parts".to_string(),
            sync: "/sync".to_string(),
            liveness: "/liveness".to_string(),
        }
    }
}

/// Errors returned by [`ChannelNames::validate`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChannelNamesError {
    #[error("channel name for `{0}` must not be empty")]
    Empty(&'static str),

    #[error("channel name `{name}` is used for both `{first}` and `{second}`")]
    Duplicate {
        name: String,
        first: &'static str,
        second: &'static str,
    },
}

impl ChannelNames {
    /// Validate that all channel names are non-empty and pairwise distinct.
    ///
    /// GossipSub and broadcast topics are derived solely from these strings,
    /// so duplicates would cause messages to be silently misrouted between
    /// logical channels (see `Channel::from_gossipsub_topic_hash`).
    pub fn validate(&self) -> Result<(), ChannelNamesError> {
        let entries = [
            ("consensus", self.consensus.as_str()),
            ("proposal_parts", self.proposal_parts.as_str()),
            ("sync", self.sync.as_str()),
            ("liveness", self.liveness.as_str()),
        ];

        for (field, name) in entries {
            if name.is_empty() {
                return Err(ChannelNamesError::Empty(field));
            }
        }

        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                if entries[i].1 == entries[j].1 {
                    return Err(ChannelNamesError::Duplicate {
                        name: entries[i].1.to_string(),
                        first: entries[i].0,
                        second: entries[j].0,
                    });
                }
            }
        }

        Ok(())
    }
}

/// Errors returned by [`P2pConfig::validate`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum P2pConfigError {
    #[error(
        "persistent peer address `{0}` is missing a /p2p/<PeerId> component; \
         without it the peer is dialed but never gets persistent-peer treatment \
         (priority, protection from pruning, gossipsub explicit-peer status)"
    )]
    PersistentPeerMissingPeerId(Multiaddr),
}

/// P2P configuration options
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct P2pConfig {
    /// Address to listen for incoming connections
    pub listen_addr: Multiaddr,

    /// List of nodes to keep persistent connections to
    pub persistent_peers: Vec<Multiaddr>,

    /// Only allow connections to/from persistent peers
    #[serde(default)]
    pub persistent_peers_only: bool,

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

    /// GossipSub / broadcast channel name configuration
    #[serde(default)]
    pub channel_names: ChannelNames,
}

impl Default for P2pConfig {
    fn default() -> Self {
        P2pConfig {
            listen_addr: Multiaddr::empty(),
            persistent_peers: vec![],
            persistent_peers_only: false,
            discovery: Default::default(),
            protocol: Default::default(),
            rpc_max_size: ByteSize::mib(10),
            pubsub_max_size: ByteSize::mib(4),
            protocol_names: Default::default(),
            channel_names: Default::default(),
        }
    }
}

impl P2pConfig {
    /// Validate that every `persistent_peers` address includes a `/p2p/<PeerId>`
    /// component.
    ///
    /// A persistent-peer address without a `PeerId` is still dialed, but silently
    /// never receives persistent-peer treatment (priority scoring, protection from
    /// pruning, gossipsub explicit-peer status) since that treatment is applied by
    /// `PeerId`, not by address — see `extract_peer_id_from_multiaddr` in
    /// `malachitebft-network`. Catching this at config-validation time turns a
    /// silent degradation into an explicit startup error.
    pub fn validate(&self) -> Result<(), P2pConfigError> {
        use multiaddr::Protocol;

        for addr in &self.persistent_peers {
            let has_peer_id = addr.iter().any(|p| matches!(p, Protocol::P2p(_)));
            if !has_peer_id {
                return Err(P2pConfigError::PersistentPeerMissingPeerId(addr.clone()));
            }
        }

        Ok(())
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
    #[serde(default = "discovery::default_num_outbound_peers")]
    pub num_outbound_peers: usize,

    /// Number of inbound peers
    #[serde(default = "discovery::default_num_inbound_peers")]
    pub num_inbound_peers: usize,

    /// Maximum number of connections per peer
    #[serde(default = "discovery::default_max_connections_per_peer")]
    pub max_connections_per_peer: usize,

    /// Maximum connections allowed per IP address.
    /// Prevents DoS attacks where an attacker generates many PeerIds from the same IP.
    #[serde(default = "discovery::default_max_connections_per_ip")]
    pub max_connections_per_ip: usize,

    /// Minimum time between reconnections from the same IP address.
    /// After all connections from an IP close, new inbound connections are rejected
    /// until this duration has elapsed. Persistent peer IPs are exempt.
    #[serde(default = "discovery::default_ip_throttle_duration")]
    #[serde(with = "humantime_serde")]
    pub ip_throttle_duration: Duration,

    /// Ephemeral connection timeout
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    pub ephemeral_connection_timeout: Duration,

    #[serde(default = "discovery::default_dial_max_retries")]
    pub dial_max_retries: usize,

    #[serde(default = "discovery::default_request_max_retries")]
    pub request_max_retries: usize,

    #[serde(default = "discovery::default_connect_request_max_retries")]
    pub connect_request_max_retries: usize,

    /// Maximum number of peer records to process or send per peers request/response.
    #[serde(default = "discovery::default_max_peers_per_response")]
    pub max_peers_per_response: usize,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        DiscoveryConfig {
            enabled: false,
            bootstrap_protocol: Default::default(),
            selector: Default::default(),
            num_outbound_peers: discovery::default_num_outbound_peers(),
            num_inbound_peers: discovery::default_num_inbound_peers(),
            max_connections_per_ip: discovery::default_max_connections_per_ip(),
            ip_throttle_duration: discovery::default_ip_throttle_duration(),
            max_connections_per_peer: discovery::default_max_connections_per_peer(),
            ephemeral_connection_timeout: Duration::from_secs(60),
            dial_max_retries: discovery::default_dial_max_retries(),
            request_max_retries: discovery::default_request_max_retries(),
            connect_request_max_retries: discovery::default_connect_request_max_retries(),
            max_peers_per_response: discovery::default_max_peers_per_response(),
        }
    }
}

mod discovery {
    use std::time::Duration;

    pub fn default_num_outbound_peers() -> usize {
        50
    }

    pub fn default_num_inbound_peers() -> usize {
        50
    }

    pub fn default_max_connections_per_peer() -> usize {
        5
    }

    pub fn default_max_connections_per_ip() -> usize {
        5
    }

    pub fn default_dial_max_retries() -> usize {
        5
    }

    pub fn default_request_max_retries() -> usize {
        5
    }

    pub fn default_connect_request_max_retries() -> usize {
        3
    }

    pub fn default_max_peers_per_response() -> usize {
        100
    }

    pub fn default_ip_throttle_duration() -> Duration {
        Duration::from_secs(30)
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

    /// Enable peer scoring to prioritize nodes based on their type in mesh formation
    enable_peer_scoring: bool,

    /// Enable explicit peering for persistent peers.
    /// When enabled, persistent peers are added as explicit peers in GossipSub,
    /// meaning a node always sends and forwards messages to its explicit peers,
    /// regardless of mesh membership.
    enable_explicit_peering: bool,

    /// Enable flood publishing.
    /// When enabled the publisher sends the messages to all known peers, not just mesh peers.
    enable_flood_publish: bool,
}

impl Default for GossipSubConfig {
    fn default() -> Self {
        // Peer scoring disabled and explicit peering disabled by default, flood_publish enabled by default
        Self::new(6, 12, 4, 2, false, false, true)
    }
}

impl GossipSubConfig {
    /// Create a new, valid GossipSub configuration.
    pub fn new(
        mesh_n: usize,
        mesh_n_high: usize,
        mesh_n_low: usize,
        mesh_outbound_min: usize,
        enable_peer_scoring: bool,
        enable_explicit_peering: bool,
        enable_flood_publish: bool,
    ) -> Self {
        let mut result = Self {
            mesh_n,
            mesh_n_high,
            mesh_n_low,
            mesh_outbound_min,
            enable_peer_scoring,
            enable_explicit_peering,
            enable_flood_publish,
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

        // Both flood_publish and explicit_peering can be enabled together.
        // flood_publish sends to all known peers on publish, explicit peering ensures
        // a node always sends and forwards messages to its explicit peers,
        // regardless of mesh membership.
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

    pub fn enable_peer_scoring(&self) -> bool {
        self.enable_peer_scoring
    }

    pub fn enable_explicit_peering(&self) -> bool {
        self.enable_explicit_peering
    }

    pub fn enable_flood_publish(&self) -> bool {
        self.enable_flood_publish
    }
}

mod gossipsub {
    use super::utils::bool_from_anything;

    fn default_enable_peer_scoring() -> bool {
        false
    }

    fn default_enable_explicit_peering() -> bool {
        false
    }

    fn default_enable_flood_publish() -> bool {
        true
    }

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
        #[serde(
            default = "default_enable_peer_scoring",
            deserialize_with = "bool_from_anything"
        )]
        enable_peer_scoring: bool,
        #[serde(
            default = "default_enable_explicit_peering",
            deserialize_with = "bool_from_anything"
        )]
        enable_explicit_peering: bool,
        #[serde(
            default = "default_enable_flood_publish",
            deserialize_with = "bool_from_anything"
        )]
        enable_flood_publish: bool,
    }

    impl From<RawConfig> for super::GossipSubConfig {
        fn from(raw: RawConfig) -> Self {
            super::GossipSubConfig::new(
                raw.mesh_n,
                raw.mesh_n_high,
                raw.mesh_n_low,
                raw.mesh_outbound_min,
                raw.enable_peer_scoring,
                raw.enable_explicit_peering,
                raw.enable_flood_publish,
            )
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "load_type", rename_all = "snake_case")]
pub enum MempoolLoadType {
    #[default]
    NoLoad,
    UniformLoad(mempool_load::UniformLoadConfig),
    NonUniformLoad(mempool_load::NonUniformLoadConfig),
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

fn default_consensus_enabled() -> bool {
    true
}

fn default_queue_capacity() -> usize {
    10
}

fn default_queue_per_height_capacity() -> usize {
    500
}

fn default_wal_replay_delay() -> Duration {
    Duration::from_secs(5)
}

/// Consensus configuration options
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Enable consensus protocol participation
    ///
    /// When disabled, the node only runs the synchronization protocol
    /// and does not subscribe to consensus-related topics
    #[serde(default = "default_consensus_enabled")]
    pub enabled: bool,

    /// P2P configuration options
    pub p2p: P2pConfig,

    /// Message types that can carry values
    pub value_payload: ValuePayload,

    /// Size of the gossip input queue (number of unique heights).
    /// Controls how many unique future heights of gossip messages
    /// (votes, proposals, proposed values) can be buffered.
    /// Default: 10
    #[serde(default = "default_queue_capacity")]
    pub queue_capacity: usize,

    /// Maximum number of buffered inputs per height in the gossip input queue.
    /// Controls how many messages (votes, proposals, proposed values) can be
    /// buffered for a single future height.
    ///
    /// For a single round with `n` validators, the minimum is `2n - 1`
    /// (1 proposal + (n-1) prevotes + (n-1) precommits). Multiply by the
    /// expected number of rounds to get a practical lower bound.
    /// Default: 500
    #[serde(default = "default_queue_per_height_capacity")]
    pub queue_per_height_capacity: usize,

    /// Duration to wait before replaying the WAL on recovery.
    ///
    /// When a validator recovers from a crash, this delay gives the sync protocol
    /// time to retrieve a certificate for the crash height. If sync succeeds
    /// during this window, WAL replay is skipped entirely.
    ///
    /// Set to 0 to disable the delay and replay immediately (previous behavior).
    /// Default: 5s
    #[serde(default = "default_wal_replay_delay", with = "humantime_serde")]
    pub wal_replay_delay: Duration,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            p2p: P2pConfig::default(),
            value_payload: ValuePayload::default(),
            queue_capacity: default_queue_capacity(),
            queue_per_height_capacity: default_queue_per_height_capacity(),
            wal_replay_delay: default_wal_replay_delay(),
        }
    }
}

/// Message types required by consensus to deliver the value being proposed
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValuePayload {
    #[default]
    ProposalAndParts,
    ProposalOnly, // TODO - add small block app to test this option
}

impl ValuePayload {
    pub fn include_parts(&self) -> bool {
        match self {
            Self::ProposalOnly => false,
            Self::ProposalAndParts => true,
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
    #[serde(default, with = "humantime_serde")]
    pub target_time: Option<Duration>,
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
            target_time: None,
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
    fn discovery_config_deserializes_without_max_peers_per_response() {
        // Configs written before this field was added should still deserialize,
        // using the default value.
        let toml = r#"
            enabled = true
            bootstrap_protocol = "full"
            selector = "random"
        "#;
        let config: DiscoveryConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_peers_per_response, 100);
    }

    #[test]
    fn discovery_config_deserializes_with_max_peers_per_response() {
        let toml = r#"
            enabled = true
            max_peers_per_response = 50
        "#;
        let config: DiscoveryConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_peers_per_response, 50);
    }

    #[test]
    fn discovery_config_default_caps_connections_per_ip() {
        let config = DiscoveryConfig::default();
        assert_eq!(config.max_connections_per_ip, 5);
    }

    #[test]
    fn discovery_config_default_max_connections_per_ip_below_num_inbound_peers() {
        let config = DiscoveryConfig::default();
        assert!(config.max_connections_per_ip < config.num_inbound_peers);
    }

    #[test]
    fn discovery_config_deserializes_without_max_connections_per_ip() {
        let toml = r#"
            enabled = true
        "#;
        let config: DiscoveryConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_connections_per_ip, 5);
        assert!(config.max_connections_per_ip < config.num_inbound_peers);
    }

    #[test]
    fn log_format() {
        assert_eq!(
            LogFormat::from_str("yaml"),
            Err("Invalid log format: yaml".to_string())
        )
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
        assert_eq!(
            protocol_names.validator_proof,
            "/malachitebft-validator-proof/v1"
        );
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
            validator_proof: "/custom-validator-proof/v1".to_string(),
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
            validator_proof: "/test-network/validator-proof/v1".to_string(),
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
        value_payload = "proposal-and-parts"
        
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
        validator_proof = "/custom-network/validator-proof/v2"
        
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
        assert_eq!(
            config.p2p.protocol_names.validator_proof,
            "/custom-network/validator-proof/v2"
        );
    }

    #[test]
    fn channel_names_validate_accepts_default() {
        assert_eq!(ChannelNames::default().validate(), Ok(()));
    }

    #[test]
    fn channel_names_validate_rejects_empty() {
        let cases = [
            (
                "consensus",
                ChannelNames {
                    consensus: String::new(),
                    ..Default::default()
                },
            ),
            (
                "proposal_parts",
                ChannelNames {
                    proposal_parts: String::new(),
                    ..Default::default()
                },
            ),
            (
                "sync",
                ChannelNames {
                    sync: String::new(),
                    ..Default::default()
                },
            ),
            (
                "liveness",
                ChannelNames {
                    liveness: String::new(),
                    ..Default::default()
                },
            ),
        ];

        for (field, names) in cases {
            assert_eq!(names.validate(), Err(ChannelNamesError::Empty(field)));
        }
    }

    #[test]
    fn channel_names_validate_rejects_duplicate() {
        let cases = [
            (
                "consensus",
                "proposal_parts",
                ChannelNames {
                    consensus: "/dup".to_string(),
                    proposal_parts: "/dup".to_string(),
                    ..Default::default()
                },
            ),
            (
                "consensus",
                "sync",
                ChannelNames {
                    consensus: "/dup".to_string(),
                    sync: "/dup".to_string(),
                    ..Default::default()
                },
            ),
            (
                "consensus",
                "liveness",
                ChannelNames {
                    consensus: "/dup".to_string(),
                    liveness: "/dup".to_string(),
                    ..Default::default()
                },
            ),
            (
                "proposal_parts",
                "sync",
                ChannelNames {
                    proposal_parts: "/dup".to_string(),
                    sync: "/dup".to_string(),
                    ..Default::default()
                },
            ),
            (
                "proposal_parts",
                "liveness",
                ChannelNames {
                    proposal_parts: "/dup".to_string(),
                    liveness: "/dup".to_string(),
                    ..Default::default()
                },
            ),
            (
                "sync",
                "liveness",
                ChannelNames {
                    sync: "/dup".to_string(),
                    liveness: "/dup".to_string(),
                    ..Default::default()
                },
            ),
        ];

        for (first, second, names) in cases {
            assert_eq!(
                names.validate(),
                Err(ChannelNamesError::Duplicate {
                    name: "/dup".to_string(),
                    first,
                    second,
                })
            );
        }
    }

    #[test]
    fn p2p_config_validate_accepts_empty_persistent_peers() {
        let config = P2pConfig::default();
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn p2p_config_validate_accepts_persistent_peer_with_peer_id() {
        let addr: Multiaddr =
            "/ip4/127.0.0.1/tcp/26656/p2p/12D3KooWJvyP3VJYymTqG7eH4PM5rN4T2agk5cdNCfNyMVb1prPu"
                .parse()
                .unwrap();
        let config = P2pConfig {
            persistent_peers: vec![addr],
            ..Default::default()
        };
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn p2p_config_validate_rejects_persistent_peer_without_peer_id() {
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/26656".parse().unwrap();
        let config = P2pConfig {
            persistent_peers: vec![addr.clone()],
            ..Default::default()
        };
        assert_eq!(
            config.validate(),
            Err(P2pConfigError::PersistentPeerMissingPeerId(addr))
        );
    }

    #[test]
    fn p2p_config_validate_rejects_first_offending_peer_among_several() {
        let good: Multiaddr =
            "/ip4/127.0.0.1/tcp/26656/p2p/12D3KooWJvyP3VJYymTqG7eH4PM5rN4T2agk5cdNCfNyMVb1prPu"
                .parse()
                .unwrap();
        let bad: Multiaddr = "/ip4/10.0.0.1/tcp/26656".parse().unwrap();
        let config = P2pConfig {
            persistent_peers: vec![good, bad.clone()],
            ..Default::default()
        };
        assert_eq!(
            config.validate(),
            Err(P2pConfigError::PersistentPeerMissingPeerId(bad))
        );
    }

    #[test]
    fn channel_names_default() {
        let channel_names = ChannelNames::default();
        assert_eq!(channel_names.consensus, "/consensus");
        assert_eq!(channel_names.proposal_parts, "/proposal_parts");
        assert_eq!(channel_names.sync, "/sync");
        assert_eq!(channel_names.liveness, "/liveness");
    }

    #[test]
    fn channel_names_serde() {
        use serde_json;

        let channel_names = ChannelNames {
            consensus: "/custom/consensus".to_string(),
            proposal_parts: "/custom/proposal_parts".to_string(),
            sync: "/custom/sync".to_string(),
            liveness: "/custom/liveness".to_string(),
        };

        let json = serde_json::to_string(&channel_names).unwrap();
        let deserialized: ChannelNames = serde_json::from_str(&json).unwrap();
        assert_eq!(channel_names, deserialized);
    }

    #[test]
    fn p2p_config_with_channel_names() {
        let config = P2pConfig::default();
        assert_eq!(config.channel_names, ChannelNames::default());

        let custom_channel_names = ChannelNames {
            consensus: "/app/consensus/v1".to_string(),
            proposal_parts: "/app/proposal_parts/v1".to_string(),
            sync: "/app/sync/v1".to_string(),
            liveness: "/app/liveness/v1".to_string(),
        };

        let config_with_custom = P2pConfig {
            channel_names: custom_channel_names.clone(),
            ..Default::default()
        };

        assert_eq!(config_with_custom.channel_names, custom_channel_names);
    }

    #[test]
    fn channel_names_toml_deserialization() {
        let toml_content = r#"
        timeout_propose = "3s"
        timeout_propose_delta = "500ms"
        timeout_prevote = "1s"
        timeout_prevote_delta = "500ms"
        timeout_precommit = "1s"
        timeout_precommit_delta = "500ms"
        timeout_rebroadcast = "5s"
        value_payload = "proposal-and-parts"

        [p2p]
        listen_addr = "/ip4/0.0.0.0/tcp/0"
        persistent_peers = []
        pubsub_max_size = "4 MiB"
        rpc_max_size = "10 MiB"

        [p2p.channel_names]
        consensus = "/custom/consensus/v2"
        proposal_parts = "/custom/proposal_parts/v2"
        sync = "/custom/sync/v2"
        liveness = "/custom/liveness/v2"

        [p2p.protocol]
        type = "gossipsub"
        "#;

        let config: ConsensusConfig = toml::from_str(toml_content).unwrap();

        assert_eq!(config.p2p.channel_names.consensus, "/custom/consensus/v2");
        assert_eq!(
            config.p2p.channel_names.proposal_parts,
            "/custom/proposal_parts/v2"
        );
        assert_eq!(config.p2p.channel_names.sync, "/custom/sync/v2");
        assert_eq!(config.p2p.channel_names.liveness, "/custom/liveness/v2");
    }

    #[test]
    fn channel_names_toml_defaults_when_missing() {
        let toml_content = r#"
        timeout_propose = "3s"
        timeout_propose_delta = "500ms"
        timeout_prevote = "1s"
        timeout_prevote_delta = "500ms"
        timeout_precommit = "1s"
        timeout_precommit_delta = "500ms"
        timeout_rebroadcast = "5s"
        value_payload = "proposal-and-parts"

        [p2p]
        listen_addr = "/ip4/0.0.0.0/tcp/0"
        persistent_peers = []
        pubsub_max_size = "4 MiB"
        rpc_max_size = "10 MiB"

        [p2p.protocol]
        type = "gossipsub"
        "#;

        let config: ConsensusConfig = toml::from_str(toml_content).unwrap();

        // Should use defaults when channel_names section is missing
        assert_eq!(config.p2p.channel_names, ChannelNames::default());
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
        value_payload = "proposal-and-parts"
        
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

    #[test]
    fn p2p_config_persistent_peers_only_default() {
        let config = P2pConfig::default();
        assert!(
            !config.persistent_peers_only,
            "persistent_peers_only should default to false"
        );
    }

    #[test]
    fn p2p_config_persistent_peers_only_toml() {
        let toml_content = r#"
        timeout_propose = "3s"
        timeout_propose_delta = "500ms"
        timeout_prevote = "1s"
        timeout_prevote_delta = "500ms"
        timeout_precommit = "1s"
        timeout_precommit_delta = "500ms"
        timeout_rebroadcast = "5s"
        value_payload = "proposal-and-parts"
        
        [p2p]
        listen_addr = "/ip4/0.0.0.0/tcp/0"
        persistent_peers = []
        persistent_peers_only = true
        pubsub_max_size = "4 MiB"
        rpc_max_size = "10 MiB"
        
        [p2p.protocol]
        type = "gossipsub"
        "#;

        let config: ConsensusConfig = toml::from_str(toml_content).unwrap();
        assert!(
            config.p2p.persistent_peers_only,
            "persistent_peers_only should be true when set in TOML"
        );
    }

    #[test]
    fn gossipsub_config_default_disables_peer_scoring() {
        let config = GossipSubConfig::default();
        assert!(!config.enable_peer_scoring());
    }

    #[test]
    fn gossipsub_enable_peer_scoring_deserialization() {
        struct TestCase {
            name: &'static str,
            toml: &'static str,
            expected: bool,
        }

        let cases = [
            TestCase {
                name: "missing field defaults to false",
                toml: r#"
                    [p2p.protocol]
                    type = "gossipsub"
                "#,
                expected: false,
            },
            TestCase {
                name: "explicit true",
                toml: r#"
                    [p2p.protocol]
                    type = "gossipsub"
                    enable_peer_scoring = true
                "#,
                expected: true,
            },
            TestCase {
                name: "explicit false",
                toml: r#"
                    [p2p.protocol]
                    type = "gossipsub"
                    enable_peer_scoring = false
                "#,
                expected: false,
            },
            TestCase {
                name: "string true",
                toml: r#"
                    [p2p.protocol]
                    type = "gossipsub"
                    enable_peer_scoring = "true"
                "#,
                expected: true,
            },
            TestCase {
                name: "string false",
                toml: r#"
                    [p2p.protocol]
                    type = "gossipsub"
                    enable_peer_scoring = "false"
                "#,
                expected: false,
            },
        ];

        for case in cases {
            let toml_content = format!(
                r#"
                timeout_propose = "3s"
                timeout_propose_delta = "500ms"
                timeout_prevote = "1s"
                timeout_prevote_delta = "500ms"
                timeout_precommit = "1s"
                timeout_precommit_delta = "500ms"
                timeout_rebroadcast = "5s"
                value_payload = "proposal-and-parts"
                
                [p2p]
                listen_addr = "/ip4/0.0.0.0/tcp/0"
                persistent_peers = []
                pubsub_max_size = "4 MiB"
                rpc_max_size = "10 MiB"
                {}
                "#,
                case.toml
            );

            let config: ConsensusConfig = toml::from_str(&toml_content)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {}", case.name, e));

            let PubSubProtocol::GossipSub(gossipsub) = config.p2p.protocol else {
                panic!("{}: expected GossipSub protocol", case.name);
            };

            assert_eq!(
                gossipsub.enable_peer_scoring(),
                case.expected,
                "{}: expected enable_peer_scoring = {}",
                case.name,
                case.expected
            );
        }
    }
}
