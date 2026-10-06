use std::error::Error;
use std::ops::ControlFlow;
use std::time::Duration;

use futures::StreamExt;
use itertools::Itertools;
use libp2p::metrics::{Metrics, Recorder};
use libp2p::request_response::{InboundRequestId, OutboundRequestId};
use libp2p::swarm::{self, SwarmEvent};
use libp2p::{gossipsub, identify, quic, SwarmBuilder};
use libp2p_broadcast as broadcast;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, error_span, info, trace, warn, Instrument};

use malachitebft_discovery::{self as discovery};
use malachitebft_metrics::SharedRegistry;
use malachitebft_sync::{self as sync};

pub use malachitebft_peer::PeerId;

pub use bytes::Bytes;
pub use libp2p::gossipsub::MessageId;
pub use libp2p::identity::Keypair;
pub use libp2p::Multiaddr;

pub mod behaviour;
pub mod handle;
pub mod pubsub;

mod channel;
pub use channel::{Channel, ChannelNames};

mod metrics;
use metrics::Metrics as NetworkMetrics;

mod peer_type;
pub use peer_type::PeerType;

pub mod peer_scoring;

mod utils;

mod ip_limits;
pub mod validator_proof;

// Re-export state types for external use (e.g., RPC)
pub use state::{LocalNodeInfo, PeerInfo, ValidatorInfo};

mod state;
pub use state::NetworkStateDump;
use state::State;

use behaviour::{Behaviour, NetworkEvent};
use handle::Handle;

const METRICS_PREFIX: &str = "malachitebft_network";
const DISCOVERY_METRICS_PREFIX: &str = "malachitebft_discovery";

/// Cadence of the network task's housekeeping timer.
///
/// TODO: Using 1 second for now, for faster reconnection during testing.
/// Maybe adjust via config in the future.
const PERIODIC_TICK: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq)]
pub struct ProtocolNames {
    /// Advertised as the `protocol_version` field of the node's Identify payload,
    /// not negotiated as a stream protocol — gossipsub and the channel names are
    /// independent of it. A peer advertising a different value is disconnected.
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

#[derive(Copy, Clone, Debug, Default)]
pub enum PubSubProtocol {
    /// GossipSub: a pubsub protocol based on epidemic broadcast trees
    #[default]
    GossipSub,

    /// Broadcast: a simple broadcast protocol
    Broadcast,
}

impl PubSubProtocol {
    pub fn is_gossipsub(&self) -> bool {
        matches!(self, Self::GossipSub)
    }

    pub fn is_broadcast(&self) -> bool {
        matches!(self, Self::Broadcast)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GossipSubConfig {
    pub mesh_n: usize,
    pub mesh_n_high: usize,
    pub mesh_n_low: usize,
    pub mesh_outbound_min: usize,
    pub enable_peer_scoring: bool,
    pub enable_explicit_peering: bool,
    pub enable_flood_publish: bool,
}

impl Default for GossipSubConfig {
    fn default() -> Self {
        // Tests use these defaults.
        Self {
            mesh_n: 6,
            mesh_n_high: 12,
            mesh_n_low: 4,
            mesh_outbound_min: 2,
            enable_peer_scoring: false,
            enable_explicit_peering: false,
            enable_flood_publish: true,
        }
    }
}

pub type BoxError = Box<dyn Error + Send + Sync + 'static>;

pub type DiscoveryConfig = discovery::Config;
pub type BootstrapProtocol = discovery::config::BootstrapProtocol;
pub type Selector = discovery::config::Selector;

/// Node identity bundling all node-specific information.
///
/// The consensus address is derived from the keypair in the current implementation
/// where libp2p and consensus use the same key. In the future, when using separate
/// keys (e.g., cc-signer for consensus), the address will be provided separately.
///
/// If consensus_address is None, the node will not advertise a validator address
/// and cannot become a validator.
#[derive(Clone, Debug)]
pub struct NetworkIdentity {
    pub moniker: String,
    pub keypair: Keypair,
    /// Validator info: consensus address and pre-serialized proof.
    /// If provided, the proof is sent on connection and when becoming validator.
    pub validator: Option<ValidatorIdentity>,
}

/// Validator identity with optional pre-serialized proof.
#[derive(Clone, Debug)]
pub struct ValidatorIdentity {
    /// The consensus address (used for local node metrics and validator set matching)
    pub address: String,
    /// Pre-serialized validator proof bytes for broadcasting (optional)
    pub proof_bytes: Option<Bytes>,
}

impl NetworkIdentity {
    /// Create a new NetworkIdentity.
    ///
    /// # Arguments
    /// * `moniker` - Human-readable node identifier
    /// * `keypair` - libp2p keypair for network authentication
    /// * `consensus_address` - Optional consensus address (Some = potential validator, None = full node)
    ///
    /// In the current implementation where libp2p and consensus share the same key,
    /// the address is typically derived from the keypair before calling this method.
    /// In the future with cc-signer, the consensus address will be separate.
    pub fn new(moniker: String, keypair: Keypair, consensus_address: Option<String>) -> Self {
        Self {
            moniker,
            keypair,
            validator: consensus_address.map(|address| ValidatorIdentity {
                address,
                proof_bytes: None,
            }),
        }
    }

    /// Create a new NodeIdentity for a validator node with a signed proof.
    ///
    /// # Arguments
    /// * `moniker` - Human-readable node identifier
    /// * `keypair` - libp2p keypair for network authentication
    /// * `address` - Consensus address
    /// * `proof_bytes` - Pre-serialized validator proof
    pub fn new_validator(
        moniker: String,
        keypair: Keypair,
        address: String,
        proof_bytes: Bytes,
    ) -> Self {
        Self {
            moniker,
            keypair,
            validator: Some(ValidatorIdentity {
                address,
                proof_bytes: Some(proof_bytes),
            }),
        }
    }

    /// Get the consensus address if this is a validator.
    pub fn consensus_address(&self) -> Option<&str> {
        self.validator.as_ref().map(|v| v.address.as_str())
    }
}

/// This structure contains optional application-payload limits for GossipSub topics.
///
/// Malachite adds signed-message overhead when it sets the corresponding wire limit. Topics without
/// a per-topic limit use the global wire limit in [`Config::pubsub_max_size`]. Value sync always uses
/// the Broadcast protocol, so it has no per-topic GossipSub limit.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PubSubMaxSizePerTopic {
    pub consensus: Option<usize>,
    pub proposal_parts: Option<usize>,
    pub liveness: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub listen_addr: Multiaddr,
    pub persistent_peers: Vec<Multiaddr>,
    pub persistent_peers_only: bool,
    pub discovery: DiscoveryConfig,
    pub idle_connection_timeout: Duration,
    pub transport: TransportProtocol,
    pub gossipsub: GossipSubConfig,
    pub pubsub_protocol: PubSubProtocol,
    pub channel_names: ChannelNames,
    pub rpc_max_size: usize,
    pub pubsub_max_size: usize,
    pub pubsub_max_size_per_topic: PubSubMaxSizePerTopic,
    pub sync_request_timeout: Duration,
    pub sync_max_request_size: usize,
    pub sync_parallel_requests: usize,
    pub enable_consensus: bool,
    pub enable_sync: bool,
    pub protocol_names: ProtocolNames,
}

/// This error reports that a per-topic payload limit does not fit the global wire limit.
#[derive(Copy, Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PubSubMaxSizeError {
    #[error(
        "pub-sub payload limit for `{field}` is {payload_size} B. Signed-message overhead makes it larger than `pubsub_max_size` of {global} B"
    )]
    ExceedsGlobal {
        field: &'static str,
        payload_size: usize,
        global: usize,
    },
}

impl Config {
    /// Build the `sync::Config` handed to the libp2p sync behaviour.
    ///
    /// Response size still comes from [`Self::rpc_max_size`]. Timeout, request
    /// size and the parallel-request stream budget come from the operator
    /// values stored on this config, not from `sync::Config::default()`.
    pub(crate) fn sync_transport_config(&self) -> sync::Config {
        sync::Config::default()
            .with_max_response_size(self.rpc_max_size)
            .with_request_timeout(self.sync_request_timeout)
            .with_max_request_size(self.sync_max_request_size)
            .with_parallel_requests(self.sync_parallel_requests)
    }

    fn apply_to_swarm(&self, cfg: swarm::Config) -> swarm::Config {
        cfg.with_idle_connection_timeout(self.idle_connection_timeout)
    }

    fn apply_to_quic(&self, mut cfg: quic::Config) -> quic::Config {
        // NOTE: This is set low due to quic transport not properly resetting
        // connection state when reconnecting before connection timeout.
        // See https://github.com/libp2p/rust-libp2p/issues/5097
        cfg.max_idle_timeout = 300;
        cfg.keep_alive_interval = Duration::from_millis(100);
        cfg
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TransportProtocol {
    Tcp,
    Quic,
}

impl TransportProtocol {
    pub fn from_multiaddr(multiaddr: &Multiaddr) -> Option<TransportProtocol> {
        for protocol in multiaddr.protocol_stack() {
            match protocol {
                "tcp" => return Some(TransportProtocol::Tcp),
                "quic" | "quic-v1" => return Some(TransportProtocol::Quic),
                _ => {}
            }
        }
        None
    }
}

/// Operation to perform on a persistent peer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistentPeersOp {
    /// Add a persistent peer
    Add(Multiaddr),
    /// Remove a persistent peer
    Remove(Multiaddr),
}

/// Errors that can occur during persistent peer operations
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PersistentPeerError {
    /// Peer already exists in the persistent peers list (for Add operation)
    #[error("Persistent peer already exists")]
    AlreadyExists,
    /// Peer not found in the persistent peers list (for Remove operation)
    #[error("Persistent peer not found")]
    NotFound,
    /// Network is not started
    #[error("Network not started")]
    NetworkStopped,
    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// sync event details:
///
/// peer1: sync                  peer2: network                    peer2: sync              peer1: network
/// CtrlMsg::SyncRequest       --> Event::Sync      -----------> CtrlMsg::SyncReply ------> Event::Sync
/// (peer_id, height)             (RawMessage::Request           (request_id, height)       RawMessage::Response
///                           {request_id, peer_id, request}                                {request_id, response}
///
///
/// An event that can be emitted by the gossip layer
#[derive(Clone, Debug)]
pub enum Event {
    Listening(Multiaddr),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    PeerSubscribed(PeerId, Channel),
    PeerUnsubscribed(PeerId, Channel),
    /// A consensus-channel message.
    ///
    /// The first `PeerId` is the delivering neighbor. The `Option` is the
    /// publisher declared in the message (`message.source`), or `None` when the
    /// transport carries none. They differ once a message is relayed.
    ConsensusMessage(Channel, PeerId, Option<PeerId>, Bytes),
    LivenessMessage(Channel, PeerId, Bytes),
    Sync(sync::RawMessage),
    /// libp2p reported that an outbound sync request to `peer` could not be
    /// delivered or completed (dial failure, connection closed, libp2p-level
    /// timeout, etc.).
    SyncRequestFailed {
        request_id: OutboundRequestId,
        peer: PeerId,
        reason: sync::OutboundFailureReason,
    },
    /// libp2p reported that the connection carrying an inbound sync request
    /// from `peer` closed before a response was sent.
    SyncInboundRequestFailed {
        request_id: InboundRequestId,
        peer: PeerId,
    },
    /// A validator proof received from a peer (one-way, no response expected).
    ValidatorProofReceived {
        peer_id: PeerId,
        proof_bytes: Bytes,
    },
}

#[derive(Debug)]
pub enum CtrlMsg {
    Publish(Channel, Bytes),
    Broadcast(Channel, Bytes),
    SyncRequest(PeerId, Bytes, oneshot::Sender<OutboundRequestId>),
    SyncReply(InboundRequestId, Bytes),
    /// Drop the response channel held for an inbound sync request that will not
    /// be answered, releasing the peer's inbound stream slot.
    SyncCancelReply(InboundRequestId),
    UpdateValidatorSet(Vec<ValidatorInfo>),
    /// Validator proof verification result. If Valid, public_key should be Some.
    /// The public_key is stored and used to check validator set membership.
    ValidatorProofVerified {
        peer_id: PeerId,
        result: validator_proof::ProofVerificationResult,
        public_key: Option<Vec<u8>>,
    },
    DumpState(oneshot::Sender<NetworkStateDump>),
    UpdatePersistentPeers(
        PersistentPeersOp,
        oneshot::Sender<Result<(), PersistentPeerError>>,
    ),
    Shutdown,
}

pub async fn spawn(
    identity: NetworkIdentity,
    config: Config,
    registry: SharedRegistry,
) -> Result<Handle, eyre::Report> {
    let mut swarm =
        registry.with_prefix(METRICS_PREFIX, |registry| -> Result<_, eyre::Report> {
            // Pass the libp2p keypair to the behaviour, it is included in the Identify protocol
            // Required for ALL nodes
            let builder =
                SwarmBuilder::with_existing_identity(identity.keypair.clone()).with_tokio();
            match config.transport {
                TransportProtocol::Tcp => {
                    let behaviour = Behaviour::new_with_metrics(&config, &identity, registry)?;
                    Ok(builder
                        .with_tcp(
                            libp2p::tcp::Config::new().nodelay(true), // Disable Nagle's algorithm
                            libp2p::noise::Config::new,
                            libp2p::yamux::Config::default,
                        )?
                        .with_dns()?
                        .with_bandwidth_metrics(registry)
                        .with_behaviour(|_| behaviour)?
                        .with_swarm_config(|cfg| config.apply_to_swarm(cfg))
                        .build())
                }
                TransportProtocol::Quic => {
                    let behaviour = Behaviour::new_with_metrics(&config, &identity, registry)?;
                    Ok(builder
                        .with_quic_config(|cfg| config.apply_to_quic(cfg))
                        .with_dns()?
                        .with_bandwidth_metrics(registry)
                        .with_behaviour(|_| behaviour)?
                        .with_swarm_config(|cfg| config.apply_to_swarm(cfg))
                        .build())
                }
            }
        })?;

    let metrics = registry.with_prefix(METRICS_PREFIX, Metrics::new);

    let (tx_event, rx_event) = mpsc::channel(32);
    let (tx_ctrl, rx_ctrl) = mpsc::channel(32);

    let discovery = registry.with_prefix(DISCOVERY_METRICS_PREFIX, |reg| {
        discovery::Discovery::new(config.discovery, config.persistent_peers.clone(), reg)
    });

    let network_metrics = registry.with_prefix(METRICS_PREFIX, NetworkMetrics::new);

    let peer_id = PeerId::from_libp2p(swarm.local_peer_id());

    // Create local node info with subscribed consensus topics
    let mut subscribed_topics = std::collections::HashSet::new();
    if config.enable_consensus {
        for channel in Channel::consensus() {
            subscribed_topics.insert(channel.as_str(&config.channel_names).to_string());
        }
    }

    let NetworkIdentity {
        moniker,
        keypair: _,
        validator,
    } = identity;

    let consensus_address = validator.as_ref().map(|v| v.address.clone());
    let proof_bytes = validator.as_ref().and_then(|v| v.proof_bytes.clone());

    // Set proof on the validator_proof behaviour so it is sent on every new connection
    if let Some(ref proof_bytes) = proof_bytes {
        if let Some(vp) = swarm.behaviour_mut().validator_proof.as_mut() {
            vp.set_proof(proof_bytes.clone());
        }
    }

    // Create local node info
    let local_node_info = LocalNodeInfo {
        moniker,
        peer_id: *swarm.local_peer_id(),
        listen_addr: config.listen_addr.clone(),
        subscribed_topics,
        consensus_address,
        proof_bytes,
        is_validator: false, // Will be updated when validator set is received
        persistent_peers_only: config.persistent_peers_only,
    };

    // Set local node info in metrics
    network_metrics.set_local_node_info(&local_node_info);

    let state = State::new(
        discovery,
        config.persistent_peers.clone(),
        local_node_info,
        network_metrics,
        config.gossipsub.enable_explicit_peering,
    );

    let span = error_span!("network");

    info!(parent: span.clone(), %peer_id, "Starting network service");

    let task_handle =
        tokio::task::spawn(run(config, metrics, state, swarm, rx_ctrl, tx_event).instrument(span));

    Ok(Handle::new(peer_id, tx_ctrl, rx_event, task_handle))
}

/// Scatter sends `Subscribe` only on a peer's first connection. Retry when a
/// further connection opens (`num_established` includes the new one), so the
/// announcement can land on a connection other than the first.
fn sync_subscribe_reannounce_on_open(enable_sync: bool, num_established: u32) -> bool {
    enable_sync && num_established > 1
}

/// Scatter does not retry when one connection closes and another remains
/// (`num_established` is the count still up). A failed oneshot closes only
/// that connection, so re-send while the peer is still connected.
fn sync_subscribe_reannounce_on_close(enable_sync: bool, num_established: u32) -> bool {
    enable_sync && num_established > 0
}

fn reannounce_sync_subscription(swarm: &mut swarm::Swarm<Behaviour>, config: &Config) {
    if let Err(e) = pubsub::subscribe(
        swarm,
        PubSubProtocol::Broadcast,
        &[Channel::Sync],
        &config.channel_names,
    ) {
        error!("Error re-announcing Sync subscribe: {e}");
    }
}

async fn run(
    config: Config,
    metrics: Metrics,
    mut state: State,
    mut swarm: swarm::Swarm<Behaviour>,
    mut rx_ctrl: mpsc::Receiver<CtrlMsg>,
    tx_event: mpsc::Sender<Event>,
) {
    // The validator proof is already set on the behaviour before run() is called
    // (see set_proof above), so it will be sent on every ConnectionEstablished.

    if let Err(e) = swarm.listen_on(config.listen_addr.clone()) {
        error!("Error listening on {}: {e}", config.listen_addr);
        return;
    }

    if config.enable_consensus {
        if let Err(e) = pubsub::subscribe(
            &mut swarm,
            config.pubsub_protocol,
            Channel::consensus(),
            &config.channel_names,
        ) {
            error!("Error subscribing to consensus channels: {e}");
            return;
        };
    }

    if config.enable_sync {
        if let Err(e) = pubsub::subscribe(
            &mut swarm,
            PubSubProtocol::Broadcast,
            &[Channel::Sync],
            &config.channel_names,
        ) {
            error!("Error subscribing to Sync channel: {e}");
            return;
        };
    }

    // Timer to perform periodic network operations (peer reconnection, metrics updates, etc.)
    let mut periodic_timer = tokio::time::interval(PERIODIC_TICK);
    let mut periodic_tick_count: u32 = 0;

    loop {
        let result = tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &config, &metrics, &mut swarm, &mut state, &tx_event).await
            }

            Some(connection_data) = state.discovery.controller.dial.recv(), if state.discovery.can_dial() => {
                state.discovery.dial_peer(&mut swarm, connection_data);
                ControlFlow::Continue(())
            }

            Some(request_data) = state.discovery.controller.peers_request.recv(), if state.discovery.can_peers_request() => {
                state.discovery.peers_request_peer(&mut swarm, request_data);
                ControlFlow::Continue(())
            }

            Some(request_data) = state.discovery.controller.connect_request.recv(), if state.discovery.can_connect_request() => {
                state.discovery.connect_request_peer(&mut swarm, request_data);
                ControlFlow::Continue(())
            }

            Some((peer_id, connection_id)) = state.discovery.controller.close.recv(), if state.discovery.can_close() => {
                state.discovery.close_connection(&mut swarm, peer_id, connection_id);
                ControlFlow::Continue(())
            }

            Some(ctrl) = rx_ctrl.recv() => {
                handle_ctrl_msg(&mut swarm, &mut state, &config, ctrl).await
            }

            _ = periodic_timer.tick() => {
                // Attempt to dial bootstrap nodes
                state.discovery.dial_bootstrap_nodes(&swarm);

                // Update peer info in State and metrics (includes gossipsub scores and mesh membership)
                if let Some(gossipsub) = swarm.behaviour_mut().gossipsub.as_mut() {
                    state.update_peer_info(
                        gossipsub,
                        Channel::consensus(),
                        &config.channel_names,
                    );
                }

                periodic_tick_count = periodic_tick_count.wrapping_add(1);
                if periodic_tick_count.is_multiple_of(5) {
                    info!("Network peer state\n{}", state.format_peer_info());
                }

                ControlFlow::Continue(())
            }
        };

        match result {
            ControlFlow::Continue(()) => continue,
            ControlFlow::Break(()) => break,
        }
    }
}

async fn handle_ctrl_msg(
    swarm: &mut swarm::Swarm<Behaviour>,
    state: &mut State,
    config: &Config,
    msg: CtrlMsg,
) -> ControlFlow<()> {
    match msg {
        CtrlMsg::Publish(channel, data) => {
            let msg_size = data.len();
            let max_size = pubsub::publish_max_payload_size(
                config.pubsub_protocol,
                channel,
                config.pubsub_max_size,
                config.pubsub_max_size_per_topic,
            );
            let result = pubsub::publish(
                swarm,
                config.pubsub_protocol,
                channel,
                &config.channel_names,
                data,
                max_size,
            );

            match result {
                Ok(()) => debug!(%channel, size = %msg_size, "Published message"),
                Err(e) => error!(%channel, "Error publishing message: {e}"),
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::Broadcast(channel, data) => {
            if channel == Channel::Sync && !config.enable_sync {
                trace!("Ignoring broadcast message to Sync channel: Sync not enabled");
                return ControlFlow::Continue(());
            }

            let msg_size = data.len();
            let max_size = pubsub::publish_max_payload_size(
                PubSubProtocol::Broadcast,
                channel,
                config.pubsub_max_size,
                config.pubsub_max_size_per_topic,
            );
            let result = pubsub::publish(
                swarm,
                PubSubProtocol::Broadcast,
                channel,
                &config.channel_names,
                data,
                max_size,
            );

            match result {
                Ok(()) => debug!(%channel, size = %msg_size, "Broadcasted message"),
                Err(e) => error!(%channel, "Error broadcasting message: {e}"),
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::SyncRequest(peer_id, request, reply_to) => {
            let Some(sync) = swarm.behaviour_mut().sync.as_mut() else {
                error!("Cannot request Sync from peer: Sync not enabled");
                return ControlFlow::Continue(());
            };

            let request_id = sync.send_request(peer_id.to_libp2p(), request);

            if let Err(e) = reply_to.send(request_id) {
                error!(%peer_id, "Error sending Sync request: {e}");
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::SyncReply(request_id, data) => {
            let Some(sync) = swarm.behaviour_mut().sync.as_mut() else {
                error!("Cannot send Sync response to peer: Sync not enabled");
                return ControlFlow::Continue(());
            };

            let Some(channel) = state.sync_channels.remove(&request_id) else {
                debug!(%request_id, "Received Sync reply for unknown request ID");
                return ControlFlow::Continue(());
            };

            let result = sync.send_response(channel, data);

            match result {
                Ok(()) => debug!(%request_id, "Replied to Sync request"),
                Err(e) => error!(%request_id, "Error replying to Sync request: {e}"),
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::SyncCancelReply(request_id) => {
            if state.sync_channels.remove(&request_id).is_some() {
                debug!(%request_id, "Dropped response channel for Sync request");
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::UpdateValidatorSet(validators) => {
            // Process the validator set update and get peers that need score updates
            let validator_set = validators.into_iter().collect();
            let changed_peers = state.process_validator_set_update(validator_set);

            // Update GossipSub scores for peers whose type changed
            for (peer_id, new_score) in &changed_peers {
                set_peer_score(swarm, *peer_id, *new_score);
            }

            // Promote newly promoted validators from ephemeral to inbound
            for (peer_id, _) in &changed_peers {
                state.try_prioritize_peer(*peer_id);
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::ValidatorProofVerified {
            peer_id,
            result,
            public_key,
        } => {
            let libp2p_peer_id = peer_id.to_libp2p();

            // Disconnect on verification failure
            if !result.is_valid() {
                warn!(%peer_id, "Invalid validator proof, disconnecting peer");
                let _ = swarm.disconnect_peer_id(libp2p_peer_id);
                return ControlFlow::Continue(());
            }

            // If signature is valid, store the proof and check validator set membership.
            // Close is handled on the swarm loop while this verdict is outstanding;
            // do not buffer a key for a peer that has already gone.
            if let Some(public_key) = public_key {
                let connected = swarm.is_connected(&libp2p_peer_id);
                if connected {
                    if let Some(new_score) =
                        state.record_verified_proof(&libp2p_peer_id, public_key)
                    {
                        set_peer_score(swarm, libp2p_peer_id, new_score);
                    }

                    // Promote newly verified validator from ephemeral to inbound
                    state.try_prioritize_peer(libp2p_peer_id);
                }
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::DumpState(reply_to) => {
            // Build a snapshot from current state
            let snapshot = NetworkStateDump {
                local_node: state.local_node.clone(),
                peers: state.peer_info.clone(),
                validator_set: state
                    .validator_set
                    .iter()
                    .cloned()
                    .sorted_unstable_by(|a, b| a.address.cmp(&b.address))
                    .collect(),
                persistent_peer_ids: state
                    .persistent_peer_ids
                    .iter()
                    .copied()
                    .sorted_unstable()
                    .collect(),
                persistent_peer_addrs: state.persistent_peer_addrs.clone(),
            };

            if let Err(_s) = reply_to.send(snapshot) {
                error!("Error replying to DumpState");
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::UpdatePersistentPeers(op, reply_to) => {
            let result = match op {
                PersistentPeersOp::Add(ref addr) => {
                    let res = state.add_persistent_peer(addr.clone(), swarm);
                    if res.is_ok() {
                        if let Some(ip) = ip_limits::extract_ip(addr) {
                            swarm.behaviour_mut().ip_limits.add_persistent_ip(ip);
                        }
                    }
                    res
                }
                PersistentPeersOp::Remove(ref addr) => {
                    let res = state.remove_persistent_peer(addr.clone(), swarm);
                    if res.is_ok() {
                        if let Some(ip) = ip_limits::extract_ip(addr) {
                            swarm.behaviour_mut().ip_limits.remove_persistent_ip(ip);
                        }
                    }
                    res
                }
            };
            if reply_to.send(result).is_err() {
                error!("Error replying to UpdatePersistentPeers");
            }
            ControlFlow::Continue(())
        }

        CtrlMsg::Shutdown => ControlFlow::Break(()),
    }
}

/// Set a default low score for a peer immediately upon connection
/// This allows gossipsub to form an initial mesh before Identify completes
fn set_default_peer_score(swarm: &mut swarm::Swarm<Behaviour>, peer_id: libp2p::PeerId) {
    if let Some(gossipsub) = swarm.behaviour_mut().gossipsub.as_mut() {
        let score = peer_scoring::get_default_score();
        gossipsub.set_application_score(&peer_id, score);
        trace!("Set default application score {score} for peer {peer_id} before Identify");
    }
}

fn set_peer_score(swarm: &mut swarm::Swarm<Behaviour>, peer_id: libp2p::PeerId, score: f64) {
    // Set application-specific score in gossipsub if enabled
    if let Some(gossipsub) = swarm.behaviour_mut().gossipsub.as_mut() {
        if gossipsub.set_application_score(&peer_id, score) {
            debug!("Upgraded application score to {score} for peer {peer_id}");
        }
    }
}

async fn handle_swarm_event(
    event: SwarmEvent<NetworkEvent>,
    config: &Config,
    metrics: &Metrics,
    swarm: &mut swarm::Swarm<Behaviour>,
    state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    if let SwarmEvent::Behaviour(NetworkEvent::GossipSub(e)) = &event {
        metrics.record(e);
    } else if let SwarmEvent::Behaviour(NetworkEvent::Identify(e)) = &event {
        metrics.record(e.as_ref());
    }

    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            debug!(%address, "Node is listening");

            if let Err(e) = tx_event.send(Event::Listening(address)).await {
                error!("Error sending listening event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        SwarmEvent::ConnectionEstablished {
            peer_id,
            connection_id,
            endpoint,
            num_established,
            ..
        } => {
            trace!("Connected to {peer_id} with connection id {connection_id}");

            // Set a low default score immediately for gossipsub mesh formation
            // This will be upgraded later when Identify completes
            let established = num_established.get();
            if established == 1 {
                // Only set score on first connection to this peer
                set_default_peer_score(swarm, peer_id);
            }

            if sync_subscribe_reannounce_on_open(config.enable_sync, established) {
                reannounce_sync_subscription(swarm, config);
            }

            state
                .discovery
                .handle_connection(swarm, peer_id, connection_id, endpoint);
        }

        SwarmEvent::OutgoingConnectionError {
            connection_id,
            error,
            ..
        } => {
            error!("Error dialing peer: {error}");

            state
                .discovery
                .handle_failed_connection(swarm, connection_id, error);
        }

        SwarmEvent::ConnectionClosed {
            peer_id,
            connection_id,
            num_established,
            cause,
            ..
        } => {
            debug!(
                "SwarmEvent::ConnectionClosed: peer_id={}, connection_id={}, num_established={}",
                peer_id, connection_id, num_established
            );
            if let Some(cause) = cause {
                warn!("Connection closed with {peer_id}, reason: {cause}");
            } else {
                warn!("Connection closed with {peer_id}, reason: unknown");
            }

            state
                .discovery
                .handle_closed_connection(swarm, peer_id, connection_id);

            if num_established == 0 {
                // Remove explicit peer before removing peer_info (needs peer_info to exist)
                state.remove_explicit_peer_from_gossipsub(swarm, &peer_id);
                state.remove_learned_persistent_peer_id(&peer_id);
                if let Some(peer_info) = state.peer_info.remove(&peer_id) {
                    state.metrics.free_slot(&peer_id, &peer_info);
                }
                // Also clean up any pending proof (proof verified before Identify completed)
                state.pending_verified_proofs.remove(&peer_id);

                if let Err(e) = tx_event
                    .send(Event::PeerDisconnected(PeerId::from_libp2p(&peer_id)))
                    .await
                {
                    error!("Error sending peer disconnected event to handle: {e}");
                    return ControlFlow::Break(());
                }
            } else if sync_subscribe_reannounce_on_close(config.enable_sync, num_established) {
                reannounce_sync_subscription(swarm, config);
            }
        }

        SwarmEvent::Behaviour(NetworkEvent::Identify(event)) => match *event {
            identify::Event::Sent { peer_id, .. } => {
                trace!("Sent identity to {peer_id}");
            }

            identify::Event::Received {
                connection_id,
                peer_id,
                info,
            } => {
                info!(
                    "Received identity from {peer_id}: protocol={:?} agent={:?}",
                    info.protocol_version, info.agent_version
                );

                if info.protocol_version == config.protocol_names.consensus {
                    trace!(
                        "Peer {peer_id} is using compatible protocol version: {:?}",
                        info.protocol_version
                    );

                    let is_already_connected = state.discovery.handle_new_peer(
                        swarm,
                        connection_id,
                        peer_id,
                        info.clone(),
                    );

                    // Update peer info in State and metrics, set peer score in gossipsub
                    let score = state.update_peer(peer_id, connection_id, &info);
                    set_peer_score(swarm, peer_id, score);

                    // Promote high-value peer (validator/persistent) from ephemeral to inbound
                    state.try_prioritize_peer(peer_id);

                    // Add persistent peers as explicit peers for guaranteed delivery
                    // (no-op when explicit peering is disabled)
                    state.add_explicit_peer_to_gossipsub(swarm, peer_id);

                    if !is_already_connected {
                        if let Err(e) = tx_event
                            .send(Event::PeerConnected(PeerId::from_libp2p(&peer_id)))
                            .await
                        {
                            error!("Error sending peer connected event to handle: {e}");
                            return ControlFlow::Break(());
                        }
                    }
                } else {
                    warn!(
                        %peer_id,
                        protocol_version = ?info.protocol_version,
                        "Incompatible protocol version, disconnecting peer"
                    );
                    let _ = swarm.disconnect_peer_id(peer_id);
                }
            }

            // Ignore other identify events
            _ => (),
        },

        SwarmEvent::Behaviour(NetworkEvent::Ping(event)) => {
            match &event.result {
                Ok(rtt) => {
                    trace!("Received pong from {} in {rtt:?}", event.peer);
                }
                Err(e) => {
                    trace!("Received pong from {} with error: {e}", event.peer);
                }
            }

            // Record metric for round-trip time sending a ping and receiving a pong
            metrics.record(&event);
        }

        SwarmEvent::Behaviour(NetworkEvent::GossipSub(event)) => {
            return handle_gossipsub_event(event, config, metrics, swarm, state, tx_event).await;
        }

        SwarmEvent::Behaviour(NetworkEvent::Broadcast(event)) => {
            return handle_broadcast_event(event, config, metrics, swarm, state, tx_event).await;
        }

        SwarmEvent::Behaviour(NetworkEvent::Sync(event)) => {
            return handle_sync_event(event, state, tx_event).await;
        }

        SwarmEvent::Behaviour(NetworkEvent::ValidatorProof(event)) => {
            return handle_validator_proof_event(event, tx_event).await;
        }

        SwarmEvent::Behaviour(NetworkEvent::Discovery(network_event)) => {
            state.discovery.on_network_event(swarm, *network_event);
        }

        swarm_event => {
            metrics.record(&swarm_event);
        }
    }

    ControlFlow::Continue(())
}

/// Build an event from an inbound GossipSub message.
///
/// `propagation_source` is the peer whose connection delivered the frame.
/// `message.source` is the publisher declared inside the message, absent in
/// anonymous mode. Both are reported; they differ once a message is relayed.
fn event_from_gossipsub_message(
    propagation_source: libp2p::PeerId,
    message_id: gossipsub::MessageId,
    message: gossipsub::Message,
    config: &Config,
) -> Option<Event> {
    let Some(channel) = Channel::from_gossipsub_topic_hash(&message.topic, &config.channel_names)
    else {
        trace!(
            "Received message {message_id} from {propagation_source} on different channel: {}",
            message.topic
        );
        return None;
    };

    let peer_id = PeerId::from_libp2p(&propagation_source);
    let published_by = message.source.as_ref().map(PeerId::from_libp2p);

    trace!(
        "Received message {message_id} from {peer_id} on channel {channel} of {} bytes",
        message.data.len()
    );

    let payload = Bytes::from(message.data);
    Some(if channel == Channel::Liveness {
        Event::LivenessMessage(channel, peer_id, payload)
    } else {
        Event::ConsensusMessage(channel, peer_id, published_by, payload)
    })
}

async fn handle_gossipsub_event(
    event: gossipsub::Event,
    config: &Config,
    _metrics: &Metrics,
    _swarm: &mut swarm::Swarm<Behaviour>,
    _state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    match event {
        gossipsub::Event::Subscribed { peer_id, topic } => {
            if !Channel::has_gossipsub_topic(&topic, &config.channel_names) {
                trace!("Peer {peer_id} tried to subscribe to unknown topic: {topic}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} subscribed to {topic}");
        }

        gossipsub::Event::Unsubscribed { peer_id, topic } => {
            if !Channel::has_gossipsub_topic(&topic, &config.channel_names) {
                trace!("Peer {peer_id} tried to unsubscribe from unknown topic: {topic}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} unsubscribed from {topic}");
        }

        gossipsub::Event::Message {
            propagation_source,
            message_id,
            message,
        } => {
            let Some(event) =
                event_from_gossipsub_message(propagation_source, message_id, message, config)
            else {
                return ControlFlow::Continue(());
            };

            if let Err(e) = tx_event.send(event).await {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        gossipsub::Event::SlowPeer {
            peer_id,
            failed_messages,
        } => {
            trace!(
                "Slow peer detected: {peer_id}, total failed messages: {}",
                failed_messages.total()
            );
        }

        gossipsub::Event::GossipsubNotSupported { peer_id } => {
            trace!("Peer does not support GossipSub: {peer_id}");
        }
    }

    ControlFlow::Continue(())
}

async fn handle_broadcast_event(
    event: broadcast::Event,
    config: &Config,
    _metrics: &Metrics,
    swarm: &mut swarm::Swarm<Behaviour>,
    _state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    match event {
        broadcast::Event::Subscribed(peer_id, topic) => {
            let Some(channel) =
                pubsub::accepted_broadcast_channel(swarm, &topic, &config.channel_names)
            else {
                trace!("Peer {peer_id} subscribed to ignored broadcast topic: {topic:?}");
                return ControlFlow::Continue(());
            };

            trace!("Peer {peer_id} subscribed to {topic:?}");

            let peer_id = PeerId::from_libp2p(&peer_id);

            if let Err(e) = tx_event.send(Event::PeerSubscribed(peer_id, channel)).await {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        broadcast::Event::Unsubscribed(peer_id, topic) => {
            let Some(channel) =
                pubsub::accepted_broadcast_channel(swarm, &topic, &config.channel_names)
            else {
                trace!("Peer {peer_id} unsubscribed from ignored broadcast topic: {topic:?}");
                return ControlFlow::Continue(());
            };

            trace!("Peer {peer_id} unsubscribed from {topic:?}");

            let peer_id = PeerId::from_libp2p(&peer_id);

            if let Err(e) = tx_event
                .send(Event::PeerUnsubscribed(peer_id, channel))
                .await
            {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        broadcast::Event::Received(peer_id, topic, message) => {
            let Some(channel) =
                pubsub::accepted_broadcast_channel(swarm, &topic, &config.channel_names)
            else {
                trace!("Received message from {peer_id} on ignored broadcast topic: {topic:?}");
                return ControlFlow::Continue(());
            };

            trace!(
                "Received message from {peer_id} on channel {channel} of {} bytes",
                message.len()
            );

            let peer_id = PeerId::from_libp2p(&peer_id);

            // Broadcast never relays, so the delivering peer is the publisher.
            let event = if channel == Channel::Liveness {
                Event::LivenessMessage(channel, peer_id, message)
            } else {
                Event::ConsensusMessage(channel, peer_id, Some(peer_id), message)
            };

            if let Err(e) = tx_event.send(event).await {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
            }
        }
    }

    ControlFlow::Continue(())
}

async fn handle_sync_event(
    event: sync::Event,
    state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    match event {
        sync::Event::Message { peer, message, .. } => {
            match message {
                libp2p::request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    state.sync_channels.insert(request_id, channel);

                    if let Err(e) = tx_event
                        .send(Event::Sync(sync::RawMessage::Request {
                            request_id,
                            peer: PeerId::from_libp2p(&peer),
                            body: request.0,
                        }))
                        .await
                    {
                        error!("Error sending Sync request to handle: {e}");
                        return ControlFlow::Break(());
                    }
                }

                libp2p::request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    if let Err(e) = tx_event
                        .send(Event::Sync(sync::RawMessage::Response {
                            request_id,
                            peer: PeerId::from_libp2p(&peer),
                            body: response.0,
                        }))
                        .await
                    {
                        error!("Error sending Sync response to handle: {e}");
                        return ControlFlow::Break(());
                    }
                }
            }

            ControlFlow::Continue(())
        }

        sync::Event::ResponseSent { .. } => ControlFlow::Continue(()),

        sync::Event::OutboundFailure {
            request_id,
            peer,
            error,
            ..
        } => {
            debug!(%request_id, %peer, ?error, "Outbound sync request failed");
            let reason = outbound_failure_reason(&error);
            if let Err(e) = tx_event
                .send(Event::SyncRequestFailed {
                    request_id,
                    peer: PeerId::from_libp2p(&peer),
                    reason,
                })
                .await
            {
                error!("Error sending sync request failure to handle: {e}");
                return ControlFlow::Break(());
            }
            ControlFlow::Continue(())
        }

        sync::Event::InboundFailure {
            request_id,
            peer,
            error,
            ..
        } => {
            debug!(%request_id, %peer, ?error, "Inbound sync request failed");
            state.sync_channels.remove(&request_id);

            if !matches!(
                error,
                libp2p::request_response::InboundFailure::ConnectionClosed
            ) {
                return ControlFlow::Continue(());
            }

            if let Err(e) = tx_event
                .send(Event::SyncInboundRequestFailed {
                    request_id,
                    peer: PeerId::from_libp2p(&peer),
                })
                .await
            {
                error!("Error sending inbound sync request failure to handle: {e}");
                return ControlFlow::Break(());
            }

            ControlFlow::Continue(())
        }
    }
}

fn outbound_failure_reason(
    error: &libp2p::request_response::OutboundFailure,
) -> sync::OutboundFailureReason {
    use libp2p::request_response::OutboundFailure;
    match error {
        OutboundFailure::DialFailure => sync::OutboundFailureReason::DialFailure,
        OutboundFailure::ConnectionClosed => sync::OutboundFailureReason::ConnectionClosed,
        OutboundFailure::Timeout => sync::OutboundFailureReason::Timeout,
        OutboundFailure::UnsupportedProtocols => sync::OutboundFailureReason::UnsupportedProtocols,
        OutboundFailure::Io(_) => sync::OutboundFailureReason::Io,
    }
}

async fn handle_validator_proof_event(
    event: validator_proof::Event,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    match event {
        validator_proof::Event::ProofReceived { peer, proof_bytes } => {
            // Forward to engine for verification
            if let Err(e) = tx_event
                .send(Event::ValidatorProofReceived {
                    peer_id: PeerId::from_libp2p(&peer),
                    proof_bytes,
                })
                .await
            {
                error!("Error sending ValidatorProofReceived to handle: {e}");
                return ControlFlow::Break(());
            }

            ControlFlow::Continue(())
        }

        validator_proof::Event::ProofSent { peer } => {
            debug!(%peer, "Validator proof sent successfully");
            ControlFlow::Continue(())
        }

        validator_proof::Event::ProofSendFailed { peer, error } => {
            debug!(%peer, %error, "Failed to send validator proof");
            ControlFlow::Continue(())
        }
    }
}

pub trait PeerIdExt {
    fn to_libp2p(&self) -> libp2p::PeerId;
    fn from_libp2p(peer_id: &libp2p::PeerId) -> Self;
}

impl PeerIdExt for PeerId {
    fn to_libp2p(&self) -> libp2p::PeerId {
        libp2p::PeerId::from_bytes(&self.to_bytes()).expect("valid PeerId")
    }

    fn from_libp2p(peer_id: &libp2p::PeerId) -> Self {
        Self::from_bytes(&peer_id.to_bytes()).expect("valid PeerId")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::{test_inbound_request_id, test_state};
    use libp2p::request_response::InboundFailure;

    fn gossip_test_config() -> Config {
        Config {
            listen_addr: Multiaddr::empty(),
            persistent_peers: vec![],
            persistent_peers_only: false,
            discovery: DiscoveryConfig::new(false),
            idle_connection_timeout: Duration::from_secs(60),
            transport: TransportProtocol::Tcp,
            gossipsub: GossipSubConfig::default(),
            pubsub_protocol: PubSubProtocol::GossipSub,
            channel_names: ChannelNames::default(),
            rpc_max_size: 10 * 1024 * 1024,
            pubsub_max_size: 4 * 1024 * 1024,
            pubsub_max_size_per_topic: Default::default(),
            sync_request_timeout: Duration::from_secs(10),
            sync_max_request_size: 1024 * 1024,
            sync_parallel_requests: 5,
            enable_consensus: true,
            enable_sync: false,
            protocol_names: ProtocolNames::default(),
        }
    }

    #[test]
    fn sync_transport_config_uses_the_operator_limits() {
        let mut config = Config {
            listen_addr: Multiaddr::empty(),
            persistent_peers: vec![],
            persistent_peers_only: false,
            discovery: DiscoveryConfig::new(false),
            idle_connection_timeout: Duration::from_secs(60),
            transport: TransportProtocol::Tcp,
            gossipsub: GossipSubConfig::default(),
            pubsub_protocol: PubSubProtocol::GossipSub,
            channel_names: ChannelNames::default(),
            rpc_max_size: 10 * 1024 * 1024,
            pubsub_max_size: 4 * 1024 * 1024,
            pubsub_max_size_per_topic: Default::default(),
            sync_request_timeout: Duration::from_secs(10),
            sync_max_request_size: 1024 * 1024,
            sync_parallel_requests: 5,
            enable_consensus: true,
            enable_sync: false,
            protocol_names: ProtocolNames::default(),
        };
        config.rpc_max_size = 3 * 1024 * 1024;
        config.sync_request_timeout = Duration::from_secs(1);
        config.sync_max_request_size = 2048;
        config.sync_parallel_requests = 7;

        let sync = config.sync_transport_config();
        assert_eq!(sync.max_response_size, 3 * 1024 * 1024);
        assert_eq!(sync.request_timeout, Duration::from_secs(1));
        assert_eq!(sync.max_request_size, 2048);
        assert_eq!(sync.parallel_requests, 7);
    }

    /// Build an `InboundFailure` sync event for `request_id`.
    fn inbound_failure_event(request_id: InboundRequestId, error: InboundFailure) -> sync::Event {
        sync::Event::InboundFailure {
            peer: libp2p::PeerId::random(),
            connection_id: libp2p::swarm::ConnectionId::new_unchecked(0),
            request_id,
            error,
        }
    }

    #[tokio::test]
    async fn handle_sync_event_reports_inbound_connection_closed() {
        let (tx_event, mut rx_event) = mpsc::channel::<Event>(1);
        let mut state = test_state();
        let request_id = test_inbound_request_id(1);

        let result = handle_sync_event(
            inbound_failure_event(request_id, InboundFailure::ConnectionClosed),
            &mut state,
            &tx_event,
        )
        .await;
        assert!(matches!(result, ControlFlow::Continue(())));

        let forwarded = rx_event.recv().await.expect("event forwarded to engine");
        match forwarded {
            Event::SyncInboundRequestFailed {
                request_id: forwarded_id,
                ..
            } => assert_eq!(forwarded_id, request_id),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn handle_sync_event_does_not_report_inbound_timeout() {
        let (tx_event, mut rx_event) = mpsc::channel::<Event>(1);
        let mut state = test_state();

        let result = handle_sync_event(
            inbound_failure_event(test_inbound_request_id(1), InboundFailure::Timeout),
            &mut state,
            &tx_event,
        )
        .await;
        assert!(matches!(result, ControlFlow::Continue(())));
        assert!(rx_event.try_recv().is_err());
    }

    #[tokio::test]
    async fn handle_sync_event_breaks_when_event_receiver_dropped() {
        let (tx_event, rx_event) = mpsc::channel::<Event>(1);
        drop(rx_event);
        let mut state = test_state();

        let result = handle_sync_event(
            inbound_failure_event(test_inbound_request_id(1), InboundFailure::ConnectionClosed),
            &mut state,
            &tx_event,
        )
        .await;
        assert!(matches!(result, ControlFlow::Break(())));
    }

    #[tokio::test]
    async fn handle_validator_proof_event_breaks_when_event_receiver_dropped() {
        let (tx_event, rx_event) = mpsc::channel::<Event>(1);
        drop(rx_event);

        let event = validator_proof::Event::ProofReceived {
            peer: libp2p::PeerId::random(),
            proof_bytes: Bytes::new(),
        };

        let result = handle_validator_proof_event(event, &tx_event).await;
        assert!(matches!(result, ControlFlow::Break(())));
    }

    #[tokio::test]
    async fn handle_validator_proof_event_forwards_proof_and_continues() {
        let (tx_event, mut rx_event) = mpsc::channel::<Event>(1);

        let peer = libp2p::PeerId::random();
        let event = validator_proof::Event::ProofReceived {
            peer,
            proof_bytes: Bytes::from_static(b"proof"),
        };

        let result = handle_validator_proof_event(event, &tx_event).await;
        assert!(matches!(result, ControlFlow::Continue(())));

        let forwarded = rx_event.recv().await.expect("event forwarded to engine");
        match forwarded {
            Event::ValidatorProofReceived {
                peer_id,
                proof_bytes,
            } => {
                assert_eq!(peer_id, PeerId::from_libp2p(&peer));
                assert_eq!(proof_bytes.as_ref(), b"proof");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn sync_subscribe_reannounce_skips_when_sync_is_disabled() {
        assert!(!sync_subscribe_reannounce_on_open(false, 2));
        assert!(!sync_subscribe_reannounce_on_close(false, 1));
    }

    #[test]
    fn sync_subscribe_reannounce_skips_the_first_and_last_connection() {
        assert!(!sync_subscribe_reannounce_on_open(true, 1));
        assert!(!sync_subscribe_reannounce_on_close(true, 0));
    }

    #[test]
    fn sync_subscribe_reannounce_retries_while_another_connection_remains() {
        assert!(sync_subscribe_reannounce_on_open(true, 2));
        assert!(sync_subscribe_reannounce_on_close(true, 1));
    }

    fn gossip_message(
        topic: gossipsub::TopicHash,
        source: Option<libp2p::PeerId>,
    ) -> gossipsub::Message {
        gossipsub::Message {
            source,
            data: b"part".to_vec(),
            sequence_number: Some(1),
            topic,
        }
    }

    #[test]
    fn gossip_event_uses_propagation_source_not_declared_source() {
        let config = gossip_test_config();
        let deliverer = libp2p::PeerId::random();
        let declared = libp2p::PeerId::random();
        let topic = Channel::ProposalParts
            .to_gossipsub_topic(&config.channel_names)
            .hash();

        let event = event_from_gossipsub_message(
            deliverer,
            gossipsub::MessageId::new(b"id"),
            gossip_message(topic, Some(declared)),
            &config,
        )
        .expect("known topic");

        match event {
            Event::ConsensusMessage(Channel::ProposalParts, from, published_by, data) => {
                assert_eq!(from, PeerId::from_libp2p(&deliverer));
                assert_ne!(from, PeerId::from_libp2p(&declared));
                assert_eq!(published_by, Some(PeerId::from_libp2p(&declared)));
                assert_eq!(data.as_ref(), b"part");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn gossip_event_attributes_anonymous_message_to_deliverer() {
        let config = gossip_test_config();
        let deliverer = libp2p::PeerId::random();
        let topic = Channel::Consensus
            .to_gossipsub_topic(&config.channel_names)
            .hash();

        let event = event_from_gossipsub_message(
            deliverer,
            gossipsub::MessageId::new(b"anon"),
            gossip_message(topic, None),
            &config,
        )
        .expect("known topic");

        match event {
            Event::ConsensusMessage(Channel::Consensus, from, published_by, data) => {
                assert_eq!(from, PeerId::from_libp2p(&deliverer));
                assert_eq!(published_by, None);
                assert_eq!(data.as_ref(), b"part");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
