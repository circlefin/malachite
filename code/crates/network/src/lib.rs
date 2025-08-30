use std::collections::HashMap;
use std::error::Error;
use std::ops::ControlFlow;
use std::time::Duration;

use futures::StreamExt;
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

use behaviour::{Behaviour, NetworkEvent};
use handle::Handle;

const METRICS_PREFIX: &str = "malachitebft_network";
const DISCOVERY_METRICS_PREFIX: &str = "malachitebft_discovery";

#[derive(Clone, Debug, PartialEq)]
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
}

impl Default for GossipSubConfig {
    fn default() -> Self {
        // Tests use these defaults.
        Self {
            mesh_n: 6,
            mesh_n_high: 12,
            mesh_n_low: 4,
            mesh_outbound_min: 2,
        }
    }
}

pub type BoxError = Box<dyn Error + Send + Sync + 'static>;

pub type DiscoveryConfig = discovery::Config;
pub type BootstrapProtocol = discovery::config::BootstrapProtocol;
pub type Selector = discovery::config::Selector;

#[derive(Clone, Debug)]
pub struct Config {
    pub listen_addr: Multiaddr,
    pub persistent_peers: Vec<Multiaddr>,
    pub discovery: DiscoveryConfig,
    pub idle_connection_timeout: Duration,
    pub transport: TransportProtocol,
    pub gossipsub: GossipSubConfig,
    pub pubsub_protocol: PubSubProtocol,
    pub channel_names: ChannelNames,
    pub rpc_max_size: usize,
    pub pubsub_max_size: usize,
    pub enable_sync: bool,
    pub protocol_names: ProtocolNames,
}

impl Config {
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
    ConsensusMessage(Channel, PeerId, Bytes),
    LivenessMessage(Channel, PeerId, Bytes),
    Sync(sync::RawMessage),
}

#[derive(Debug)]
pub enum CtrlMsg {
    Publish(Channel, Bytes),
    Broadcast(Channel, Bytes),
    SyncRequest(PeerId, Bytes, oneshot::Sender<OutboundRequestId>),
    SyncReply(InboundRequestId, Bytes),
    Shutdown,
}

#[derive(Debug)]
pub struct State {
    pub sync_channels: HashMap<InboundRequestId, sync::ResponseChannel>,
    pub discovery: discovery::Discovery<Behaviour>,
}

impl State {
    fn new(discovery: discovery::Discovery<Behaviour>) -> Self {
        Self {
            sync_channels: Default::default(),
            discovery,
        }
    }
}

pub async fn spawn(
    keypair: Keypair,
    config: Config,
    registry: SharedRegistry,
) -> Result<Handle, eyre::Report> {
    let swarm = registry.with_prefix(METRICS_PREFIX, |registry| -> Result<_, eyre::Report> {
        let builder = SwarmBuilder::with_existing_identity(keypair.clone()).with_tokio();
        match config.transport {
            TransportProtocol::Tcp => {
                let behaviour = Behaviour::new_with_metrics(&config, &keypair, registry)?;
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
                let behaviour = Behaviour::new_with_metrics(&config, &keypair, registry)?;
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

    let state = State::new(discovery);

    let peer_id = PeerId::from_libp2p(swarm.local_peer_id());
    let span = error_span!("network");

    info!(parent: span.clone(), %peer_id, "Starting network service");

    let task_handle =
        tokio::task::spawn(run(config, metrics, state, swarm, rx_ctrl, tx_event).instrument(span));

    Ok(Handle::new(peer_id, tx_ctrl, rx_event, task_handle))
}

async fn run(
    config: Config,
    metrics: Metrics,
    mut state: State,
    mut swarm: swarm::Swarm<Behaviour>,
    mut rx_ctrl: mpsc::Receiver<CtrlMsg>,
    tx_event: mpsc::Sender<Event>,
) {
    if let Err(e) = swarm.listen_on(config.listen_addr.clone()) {
        error!("Error listening on {}: {e}", config.listen_addr);
        return;
    }

    state.discovery.dial_bootstrap_nodes(&swarm);

    if let Err(e) = pubsub::subscribe(
        &mut swarm,
        config.pubsub_protocol,
        Channel::consensus(),
        config.channel_names,
    ) {
        error!("Error subscribing to consensus channels: {e}");
        return;
    };

    if config.enable_sync {
        if let Err(e) = pubsub::subscribe(
            &mut swarm,
            PubSubProtocol::Broadcast,
            &[Channel::Sync],
            config.channel_names,
        ) {
            error!("Error subscribing to Sync channel: {e}");
            return;
        };
    }

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
            let result = pubsub::publish(
                swarm,
                config.pubsub_protocol,
                channel,
                config.channel_names,
                data,
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
            let result = pubsub::publish(
                swarm,
                PubSubProtocol::Broadcast,
                channel,
                config.channel_names,
                data,
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
                error!(%request_id, "Received Sync reply for unknown request ID");
                return ControlFlow::Continue(());
            };

            let result = sync.send_response(channel, data);

            match result {
                Ok(()) => debug!(%request_id, "Replied to Sync request"),
                Err(e) => error!(%request_id, "Error replying to Sync request: {e}"),
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::Shutdown => ControlFlow::Break(()),
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
            ..
        } => {
            trace!("Connected to {peer_id} with connection id {connection_id}",);

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
            if let Some(cause) = cause {
                warn!("Connection closed with {peer_id}, reason: {cause}");
            } else {
                warn!("Connection closed with {peer_id}, reason: unknown");
            }

            state
                .discovery
                .handle_closed_connection(swarm, peer_id, connection_id);

            if num_established == 0 {
                if let Err(e) = tx_event
                    .send(Event::PeerDisconnected(PeerId::from_libp2p(&peer_id)))
                    .await
                {
                    error!("Error sending peer disconnected event to handle: {e}");
                    return ControlFlow::Break(());
                }
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
                trace!(
                    "Received identity from {peer_id}: protocol={:?}",
                    info.protocol_version
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
                    trace!(
                        "Peer {peer_id} is using incompatible protocol version: {:?}",
                        info.protocol_version
                    );
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
            return handle_sync_event(event, metrics, swarm, state, tx_event).await;
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
            if !Channel::has_gossipsub_topic(&topic, config.channel_names) {
                trace!("Peer {peer_id} tried to subscribe to unknown topic: {topic}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} subscribed to {topic}");
        }

        gossipsub::Event::Unsubscribed { peer_id, topic } => {
            if !Channel::has_gossipsub_topic(&topic, config.channel_names) {
                trace!("Peer {peer_id} tried to unsubscribe from unknown topic: {topic}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} unsubscribed from {topic}");
        }

        gossipsub::Event::Message {
            message_id,
            message,
            ..
        } => {
            let Some(peer_id) = message.source else {
                return ControlFlow::Continue(());
            };

            let Some(channel) =
                Channel::from_gossipsub_topic_hash(&message.topic, config.channel_names)
            else {
                trace!(
                    "Received message {message_id} from {peer_id} on different channel: {}",
                    message.topic
                );

                return ControlFlow::Continue(());
            };

            trace!(
                "Received message {message_id} from {peer_id} on channel {channel} of {} bytes",
                message.data.len()
            );

            let peer_id = PeerId::from_libp2p(&peer_id);

            let event = if channel == Channel::Liveness {
                Event::LivenessMessage(channel, peer_id, Bytes::from(message.data))
            } else {
                Event::ConsensusMessage(channel, peer_id, Bytes::from(message.data))
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
    _swarm: &mut swarm::Swarm<Behaviour>,
    _state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    match event {
        broadcast::Event::Subscribed(peer_id, topic) => {
            if !Channel::has_broadcast_topic(&topic, config.channel_names) {
                trace!("Peer {peer_id} tried to subscribe to unknown topic: {topic:?}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} subscribed to {topic:?}");
        }

        broadcast::Event::Unsubscribed(peer_id, topic) => {
            if !Channel::has_broadcast_topic(&topic, config.channel_names) {
                trace!("Peer {peer_id} tried to unsubscribe from unknown topic: {topic:?}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} unsubscribed from {topic:?}");
        }

        broadcast::Event::Received(peer_id, topic, message) => {
            let Some(channel) = Channel::from_broadcast_topic(&topic, config.channel_names) else {
                trace!("Received message from {peer_id} on different channel: {topic:?}");
                return ControlFlow::Continue(());
            };

            trace!(
                "Received message from {peer_id} on channel {channel} of {} bytes",
                message.len()
            );

            let peer_id = PeerId::from_libp2p(&peer_id);

            let event = if channel == Channel::Liveness {
                Event::LivenessMessage(channel, peer_id, message)
            } else {
                Event::ConsensusMessage(channel, peer_id, message)
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
    _metrics: &Metrics,
    _swarm: &mut swarm::Swarm<Behaviour>,
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

                    let _ = tx_event
                        .send(Event::Sync(sync::RawMessage::Request {
                            request_id,
                            peer: PeerId::from_libp2p(&peer),
                            body: request.0,
                        }))
                        .await
                        .map_err(|e| {
                            error!("Error sending Sync request to handle: {e}");
                        });
                }

                libp2p::request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    let _ = tx_event
                        .send(Event::Sync(sync::RawMessage::Response {
                            request_id,
                            peer: PeerId::from_libp2p(&peer),
                            body: response.0,
                        }))
                        .await
                        .map_err(|e| {
                            error!("Error sending Sync response to handle: {e}");
                        });
                }
            }

            ControlFlow::Continue(())
        }

        sync::Event::ResponseSent { .. } => ControlFlow::Continue(()),

        sync::Event::OutboundFailure { .. } => ControlFlow::Continue(()),

        sync::Event::InboundFailure { .. } => ControlFlow::Continue(()),
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
