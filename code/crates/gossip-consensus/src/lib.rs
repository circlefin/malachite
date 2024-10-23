// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::collections::HashMap;
use std::error::Error;
use std::ops::ControlFlow;
use std::time::Duration;

use futures::StreamExt;
use libp2p::metrics::{Metrics, Recorder};
use libp2p::request_response::InboundRequestId;
use libp2p::swarm::{self, SwarmEvent};
use libp2p::{gossipsub, identify, quic, SwarmBuilder};
use libp2p_broadcast as broadcast;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, error_span, trace, Instrument};

use malachite_blocksync::{self as blocksync, OutboundRequestId};
use malachite_discovery::{self as discovery, ConnectionData};
use malachite_metrics::SharedRegistry;

pub use bytes::Bytes;
pub use libp2p::gossipsub::MessageId;
pub use libp2p::identity::Keypair;
pub use libp2p::{Multiaddr, PeerId};

pub mod behaviour;
pub mod handle;
pub mod pubsub;

mod channel;
pub use channel::Channel;

use behaviour::{Behaviour, NetworkEvent};
use handle::Handle;

const PROTOCOL: &str = "/malachite-consensus/v1beta1";
const METRICS_PREFIX: &str = "malachite_gossip_consensus";
const DISCOVERY_METRICS_PREFIX: &str = "malachite_discovery";

#[derive(Copy, Clone, Debug)]
pub enum PubSubProtocol {
    /// GossipSub: a pubsub protocol based on epidemic broadcast trees
    GossipSub(GossipSubConfig),

    /// Broadcast: a simple broadcast protocol
    Broadcast,
}

impl PubSubProtocol {
    pub fn is_gossipsub(&self) -> bool {
        matches!(self, Self::GossipSub(_))
    }

    pub fn is_broadcast(&self) -> bool {
        matches!(self, Self::Broadcast)
    }
}

impl Default for PubSubProtocol {
    fn default() -> Self {
        Self::GossipSub(GossipSubConfig::default())
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

#[derive(Clone, Debug)]
pub struct Config {
    pub listen_addr: Multiaddr,
    pub persistent_peers: Vec<Multiaddr>,
    pub discovery: DiscoveryConfig,
    pub idle_connection_timeout: Duration,
    pub transport: TransportProtocol,
    pub protocol: PubSubProtocol,
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

/// Blocksync event details:
///
/// peer1: blocksync              peer2: gossip_consensus        peer2: blocksync                peer1: gossip_consensus
/// CtrlMsg::BlockSyncRequest --> Event::BlockSync  -----------> CtrlMsg::BlockSyncReply ------> Event::BlockSync
/// (peer_id, height)             (RawMessage::Request           (request_id, height)            RawMessage::Response
///                             {request_id, peer_id, height}                                    {request_id, block}
///
/// An event that can be emitted by the gossip layer
#[derive(Clone, Debug)]
pub enum Event {
    Listening(Multiaddr),
    Message(Channel, PeerId, Bytes),
    BlockSync(blocksync::RawMessage),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

#[derive(Debug)]
pub enum CtrlMsg {
    Publish(Channel, Bytes),
    BlockSyncRequest(PeerId, Bytes, oneshot::Sender<OutboundRequestId>),
    BlockSyncReply(InboundRequestId, Bytes),
    Shutdown,
}

#[derive(Debug)]
pub struct State {
    pub blocksync_channels: HashMap<InboundRequestId, blocksync::ResponseChannel>,
    pub discovery: discovery::Discovery,
}

impl State {
    fn new(discovery: discovery::Discovery) -> Self {
        Self {
            blocksync_channels: Default::default(),
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
        let builder = SwarmBuilder::with_existing_identity(keypair).with_tokio();
        match config.transport {
            TransportProtocol::Tcp => Ok(builder
                .with_tcp(
                    libp2p::tcp::Config::new().nodelay(true), // Disable Nagle's algorithm
                    libp2p::noise::Config::new,
                    libp2p::yamux::Config::default,
                )?
                .with_dns()?
                .with_bandwidth_metrics(registry)
                .with_behaviour(|kp| {
                    Behaviour::new_with_metrics(config.protocol, kp, config.discovery, registry)
                })?
                .with_swarm_config(|cfg| config.apply_to_swarm(cfg))
                .build()),
            TransportProtocol::Quic => Ok(builder
                .with_quic_config(|cfg| config.apply_to_quic(cfg))
                .with_dns()?
                .with_bandwidth_metrics(registry)
                .with_behaviour(|kp| {
                    Behaviour::new_with_metrics(config.protocol, kp, config.discovery, registry)
                })?
                .with_swarm_config(|cfg| config.apply_to_swarm(cfg))
                .build()),
        }
    })?;

    let metrics = registry.with_prefix(METRICS_PREFIX, Metrics::new);

    let (tx_event, rx_event) = mpsc::channel(32);
    let (tx_ctrl, rx_ctrl) = mpsc::channel(32);

    let discovery = registry.with_prefix(DISCOVERY_METRICS_PREFIX, |reg| {
        discovery::Discovery::new(config.discovery, config.persistent_peers.clone(), reg)
    });

    let state = State::new(discovery);

    let peer_id = *swarm.local_peer_id();
    let span = error_span!("gossip.consensus", peer = %peer_id);
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
    };

    for persistent_peer in config.persistent_peers {
        state
            .discovery
            .add_to_dial_queue(&swarm, ConnectionData::new(None, persistent_peer));
    }

    if let Err(e) = pubsub::subscribe(&mut swarm, Channel::all()) {
        error!("Error subscribing to channels: {e}");
        return;
    };

    loop {
        let result = tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &metrics, &mut swarm, &mut state, &tx_event).await
            }

            Some(connection_data) = state.discovery.rx_dial.recv(), if state.discovery.can_dial() => {
                state.discovery.dial_peer(&mut swarm, connection_data);
                ControlFlow::Continue(())
            }

            Some(request_data) = state.discovery.rx_request.recv(), if state.discovery.can_request() => {
                state.discovery.request_peer(&mut swarm, request_data);
                ControlFlow::Continue(())
            }

            Some(ctrl) = rx_ctrl.recv() => {
                handle_ctrl_msg(ctrl, &mut swarm, &mut state).await
            }
        };

        match result {
            ControlFlow::Continue(()) => continue,
            ControlFlow::Break(()) => break,
        }
    }
}

async fn handle_ctrl_msg(
    msg: CtrlMsg,
    swarm: &mut swarm::Swarm<Behaviour>,
    state: &mut State,
) -> ControlFlow<()> {
    match msg {
        CtrlMsg::Publish(channel, data) => {
            let msg_size = data.len();
            let result = pubsub::publish(swarm, channel, data);

            match result {
                Ok(()) => debug!(%channel, size = %msg_size, "Published message"),
                Err(e) => error!(%channel, "Error broadcasting message: {e}"),
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::BlockSyncRequest(peer_id, request, reply_to) => {
            let request_id = swarm
                .behaviour_mut()
                .blocksync
                .send_request(peer_id, request);

            if let Err(e) = reply_to.send(request_id) {
                error!(%peer_id, "Error sending BlockSync request: {e}");
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::BlockSyncReply(request_id, data) => {
            let Some(channel) = state.blocksync_channels.remove(&request_id) else {
                error!(%request_id, "Received BlockSync reply for unknown request ID");
                return ControlFlow::Continue(());
            };

            let result = swarm.behaviour_mut().blocksync.send_response(channel, data);

            match result {
                Ok(()) => trace!("Replied to BlockSync request"),
                Err(e) => error!("Error replying to BlockSync request: {e}"),
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::Shutdown => ControlFlow::Break(()),
    }
}

async fn handle_swarm_event(
    event: SwarmEvent<NetworkEvent>,
    metrics: &Metrics,
    swarm: &mut swarm::Swarm<Behaviour>,
    state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    if let SwarmEvent::Behaviour(NetworkEvent::GossipSub(e)) = &event {
        metrics.record(e);
    } else if let SwarmEvent::Behaviour(NetworkEvent::Identify(e)) = &event {
        metrics.record(e);
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
            state
                .discovery
                .handle_connection(peer_id, connection_id, endpoint);
        }

        SwarmEvent::OutgoingConnectionError {
            connection_id,
            error,
            ..
        } => {
            error!("Error dialing peer: {error}");
            state.discovery.handle_failed_connection(connection_id);
        }

        SwarmEvent::ConnectionClosed {
            peer_id,
            connection_id,
            cause,
            ..
        } => {
            error!("Connection closed with {peer_id}: {:?}", cause);
            state.discovery.remove_peer(peer_id, connection_id);
        }

        SwarmEvent::Behaviour(NetworkEvent::Identify(identify::Event::Sent {
            peer_id, ..
        })) => {
            trace!("Sent identity to {peer_id}");
        }

        SwarmEvent::Behaviour(NetworkEvent::Identify(identify::Event::Received {
            connection_id,
            peer_id,
            info,
        })) => {
            trace!(
                "Received identity from {peer_id}: protocol={:?}",
                info.protocol_version
            );

            if info.protocol_version == PROTOCOL {
                trace!(
                    "Peer {peer_id} is using compatible protocol version: {:?}",
                    info.protocol_version
                );

                state
                    .discovery
                    .handle_new_peer(connection_id, peer_id, info)
            } else {
                trace!(
                    "Peer {peer_id} is using incompatible protocol version: {:?}",
                    info.protocol_version
                );
            }
        }

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
            return handle_gossipsub_event(event, metrics, swarm, state, tx_event).await;
        }

        SwarmEvent::Behaviour(NetworkEvent::Broadcast(event)) => {
            return handle_broadcast_event(event, metrics, swarm, state, tx_event).await;
        }

        SwarmEvent::Behaviour(NetworkEvent::BlockSync(event)) => {
            return handle_blocksync_event(event, metrics, swarm, state, tx_event).await;
        }

        SwarmEvent::Behaviour(NetworkEvent::RequestResponse(event)) => {
            state.discovery.on_event(event, swarm);
        }

        swarm_event => {
            metrics.record(&swarm_event);
        }
    }

    ControlFlow::Continue(())
}

async fn handle_gossipsub_event(
    event: gossipsub::Event,
    _metrics: &Metrics,
    _swarm: &mut swarm::Swarm<Behaviour>,
    _state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    match event {
        gossipsub::Event::Subscribed { peer_id, topic } => {
            if !Channel::has_gossipsub_topic(&topic) {
                trace!("Peer {peer_id} tried to subscribe to unknown topic: {topic}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} subscribed to {topic}");

            if let Err(e) = tx_event.send(Event::PeerConnected(peer_id)).await {
                error!("Error sending peer connected event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        gossipsub::Event::Unsubscribed { peer_id, topic } => {
            if !Channel::has_gossipsub_topic(&topic) {
                trace!("Peer {peer_id} tried to unsubscribe from unknown topic: {topic}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} unsubscribed from {topic}");

            if let Err(e) = tx_event.send(Event::PeerDisconnected(peer_id)).await {
                error!("Error sending peer disconnected event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        gossipsub::Event::Message {
            message_id,
            message,
            ..
        } => {
            let Some(peer_id) = message.source else {
                return ControlFlow::Continue(());
            };

            let Some(channel) = Channel::from_gossipsub_topic_hash(&message.topic) else {
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

            let event = Event::Message(channel, peer_id, Bytes::from(message.data));

            if let Err(e) = tx_event.send(event).await {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
            }
        }
        gossipsub::Event::GossipsubNotSupported { peer_id } => {
            trace!("Peer {peer_id} does not support GossipSub");
        }
    }

    ControlFlow::Continue(())
}

async fn handle_broadcast_event(
    event: broadcast::Event,
    _metrics: &Metrics,
    _swarm: &mut swarm::Swarm<Behaviour>,
    _state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    match event {
        broadcast::Event::Subscribed(peer_id, topic) => {
            if !Channel::has_broadcast_topic(&topic) {
                trace!("Peer {peer_id} tried to subscribe to unknown topic: {topic:?}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} subscribed to {topic:?}");

            if let Err(e) = tx_event.send(Event::PeerConnected(peer_id)).await {
                error!("Error sending peer connected event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        broadcast::Event::Unsubscribed(peer_id, topic) => {
            if !Channel::has_broadcast_topic(&topic) {
                trace!("Peer {peer_id} tried to unsubscribe from unknown topic: {topic:?}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} unsubscribed from {topic:?}");

            if let Err(e) = tx_event.send(Event::PeerDisconnected(peer_id)).await {
                error!("Error sending peer disconnected event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        broadcast::Event::Received(peer_id, topic, message) => {
            let Some(channel) = Channel::from_broadcast_topic(&topic) else {
                trace!("Received message from {peer_id} on different channel: {topic:?}");
                return ControlFlow::Continue(());
            };

            trace!(
                "Received message from {peer_id} on channel {channel} of {} bytes",
                message.len()
            );

            let event = Event::Message(channel, peer_id, Bytes::copy_from_slice(message.as_ref()));

            if let Err(e) = tx_event.send(event).await {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
            }
        }
    }

    ControlFlow::Continue(())
}

async fn handle_blocksync_event(
    event: blocksync::Event,
    _metrics: &Metrics,
    _swarm: &mut swarm::Swarm<Behaviour>,
    state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    match event {
        blocksync::Event::Message { peer, message } => {
            match message {
                libp2p::request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    state.blocksync_channels.insert(request_id, channel);

                    let _ = tx_event
                        .send(Event::BlockSync(blocksync::RawMessage::Request {
                            request_id,
                            peer,
                            body: request.0,
                        }))
                        .await
                        .map_err(|e| {
                            error!("Error sending BlockSync request to handle: {e}");
                        });
                }

                libp2p::request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    let _ = tx_event
                        .send(Event::BlockSync(blocksync::RawMessage::Response {
                            request_id,
                            body: response.0,
                        }))
                        .await
                        .map_err(|e| {
                            error!("Error sending BlockSync response to handle: {e}");
                        });
                }
            }
            ControlFlow::Continue(())
        }

        blocksync::Event::ResponseSent { peer, request_id } => {
            // TODO
            let _ = (peer, request_id);
            ControlFlow::Continue(())
        }

        blocksync::Event::OutboundFailure {
            peer,
            request_id,
            error,
        } => {
            let _ = (peer, request_id, error);
            ControlFlow::Continue(())
        }

        blocksync::Event::InboundFailure {
            peer,
            request_id,
            error,
        } => {
            let _ = (peer, request_id, error);
            ControlFlow::Continue(())
        }
    }
}
