#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use core::fmt;
use std::collections::HashMap;
use std::error::Error;
use std::ops::ControlFlow;
use std::time::Duration;

use eyre::eyre;
use futures::StreamExt;
use libp2p::metrics::{Metrics, Recorder};
use libp2p::swarm::{self, SwarmEvent};
use libp2p::{gossipsub, identify, SwarmBuilder};
use prost::bytes::Bytes;
use tokio::sync::mpsc;
use tracing::{debug, error, error_span, trace, Instrument};

use malachitebft_metrics::SharedRegistry;

pub use libp2p::gossipsub::MessageId;
pub use libp2p::identity::Keypair;
pub use libp2p::{Multiaddr, PeerId};

pub mod behaviour;
pub mod handle;
pub mod proto;
pub mod types;

mod msg;
pub use msg::NetworkMsg;

use behaviour::{Behaviour, NetworkEvent};
use handle::Handle;

const METRICS_PREFIX: &str = "malachitebft_test_mempool";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Channel {
    Mempool,
}

impl Channel {
    pub fn all() -> &'static [Channel] {
        &[Channel::Mempool]
    }

    pub fn to_topic(self) -> gossipsub::IdentTopic {
        gossipsub::IdentTopic::new(self.as_str())
    }

    pub fn topic_hash(&self) -> gossipsub::TopicHash {
        self.to_topic().hash()
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Channel::Mempool => "/mempool",
        }
    }

    pub fn has_topic(topic_hash: &gossipsub::TopicHash) -> bool {
        Self::all()
            .iter()
            .any(|channel| &channel.topic_hash() == topic_hash)
    }

    pub fn from_topic_hash(topic: &gossipsub::TopicHash) -> Option<Self> {
        match topic.as_str() {
            "/mempool" => Some(Channel::Mempool),
            _ => None,
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

const PROTOCOL: &str = "/malachitebft-test-mempool/v1beta1";

pub type BoxError = Box<dyn Error + Send + Sync + 'static>;

#[derive(Clone, Debug)]
pub struct Config {
    pub listen_addr: Multiaddr,
    pub persistent_peers: Vec<Multiaddr>,
    pub idle_connection_timeout: Duration,
}

impl Config {
    fn apply(&self, cfg: swarm::Config) -> swarm::Config {
        cfg.with_idle_connection_timeout(self.idle_connection_timeout)
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
                "quic" => return Some(TransportProtocol::Quic),
                _ => {}
            }
        }
        None
    }
}

/// An event that can be emitted by the gossip layer
#[derive(Clone, Debug)]
pub enum Event {
    Listening(Multiaddr),
    Message(Channel, PeerId, MessageId, NetworkMsg),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

#[derive(Debug)]
pub enum CtrlMsg {
    BroadcastMsg(Channel, Bytes),
    Shutdown,
}

#[derive(Debug, Default)]
pub struct State {
    pub peers: HashMap<PeerId, identify::Info>,
}

pub async fn spawn(
    keypair: Keypair,
    config: Config,
    registry: SharedRegistry,
) -> Result<Handle, BoxError> {
    let mut swarm = registry.with_prefix(METRICS_PREFIX, |registry| -> Result<_, BoxError> {
        let builder = SwarmBuilder::with_existing_identity(keypair).with_tokio();
        match TransportProtocol::from_multiaddr(&config.listen_addr) {
            Some(TransportProtocol::Tcp) => Ok(builder
                .with_tcp(
                    libp2p::tcp::Config::new().nodelay(true), // Disable Nagle's algorithm
                    libp2p::noise::Config::new,
                    libp2p::yamux::Config::default,
                )?
                .with_dns()?
                .with_bandwidth_metrics(registry)
                .with_behaviour(|kp| Behaviour::new_with_metrics(kp, registry))?
                .with_swarm_config(|cfg| config.apply(cfg))
                .build()),
            Some(TransportProtocol::Quic) => Ok(builder
                .with_quic()
                .with_dns()?
                .with_bandwidth_metrics(registry)
                .with_behaviour(|kp| Behaviour::new_with_metrics(kp, registry))?
                .with_swarm_config(|cfg| config.apply(cfg))
                .build()),

            None => Err(eyre!(
                "No valid transport protocol found in listen address: {}",
                config.listen_addr
            )
            .into()),
        }
    })?;

    for channel in Channel::all() {
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&channel.to_topic())?;
    }

    let metrics = registry.with_prefix(METRICS_PREFIX, Metrics::new);

    let (tx_event, rx_event) = mpsc::channel(32);
    let (tx_ctrl, rx_ctrl) = mpsc::channel(32);

    let peer_id = swarm.local_peer_id();
    let span = error_span!("mempool.network", peer = %peer_id);
    let task_handle =
        tokio::task::spawn(run(config, metrics, swarm, rx_ctrl, tx_event).instrument(span));

    Ok(Handle::new(tx_ctrl, rx_event, task_handle))
}

async fn run(
    config: Config,
    metrics: Metrics,
    mut swarm: swarm::Swarm<Behaviour>,
    mut rx_ctrl: mpsc::Receiver<CtrlMsg>,
    tx_event: mpsc::Sender<Event>,
) {
    if let Err(e) = swarm.listen_on(config.listen_addr.clone()) {
        error!("Error listening on {}: {e}", config.listen_addr);
        return;
    };

    for persistent_peer in config.persistent_peers {
        trace!("Dialing persistent peer: {persistent_peer}");

        match swarm.dial(persistent_peer.clone()) {
            Ok(()) => (),
            Err(e) => error!("Error dialing persistent peer {persistent_peer}: {e}"),
        }
    }

    let mut state = State::default();

    loop {
        let result = tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &metrics, &mut swarm, &mut state, &tx_event).await
            }

            Some(ctrl) = rx_ctrl.recv() => {
                handle_ctrl_msg(ctrl, &mut swarm).await
            }
        };

        match result {
            ControlFlow::Continue(()) => continue,
            ControlFlow::Break(()) => break,
        }
    }
}

async fn handle_ctrl_msg(msg: CtrlMsg, swarm: &mut swarm::Swarm<Behaviour>) -> ControlFlow<()> {
    match msg {
        CtrlMsg::BroadcastMsg(channel, data) => {
            let msg_size = data.len();

            let result = swarm
                .behaviour_mut()
                .gossipsub
                .publish(channel.topic_hash(), data);

            match result {
                Ok(message_id) => {
                    debug!(
                        %channel,
                        id = %message_id,
                        size = %msg_size,
                        "Broadcasted message"
                    );
                }
                Err(e) => {
                    error!(%channel, "Error broadcasting message: {e}");
                }
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::Shutdown => ControlFlow::Break(()),
    }
}

async fn handle_swarm_event(
    event: SwarmEvent<NetworkEvent>,
    metrics: &Metrics,
    _swarm: &mut swarm::Swarm<Behaviour>,
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

        SwarmEvent::Behaviour(NetworkEvent::Identify(identify::Event::Sent {
            peer_id, ..
        })) => {
            trace!("Sent identity to {peer_id}");
        }

        SwarmEvent::Behaviour(NetworkEvent::Identify(identify::Event::Received {
            peer_id,
            info,
            ..
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

                state.peers.insert(peer_id, info);
            } else {
                trace!(
                    "Peer {peer_id} is using incompatible protocol version: {:?}",
                    info.protocol_version
                );
            }
        }

        SwarmEvent::Behaviour(NetworkEvent::GossipSub(gossipsub::Event::Subscribed {
            peer_id,
            topic,
        })) => {
            if !Channel::has_topic(&topic) {
                trace!("Peer {peer_id} tried to subscribe to unknown topic: {topic}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} subscribed to {topic}");
        }

        SwarmEvent::Behaviour(NetworkEvent::GossipSub(gossipsub::Event::Unsubscribed {
            peer_id,
            topic,
        })) => {
            if !Channel::has_topic(&topic) {
                trace!("Peer {peer_id} tried to unsubscribe from unknown topic: {topic}");
                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} unsubscribed from {topic}");

            if let Err(e) = tx_event.send(Event::PeerDisconnected(peer_id)).await {
                error!("Error sending peer disconnected event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        SwarmEvent::Behaviour(NetworkEvent::GossipSub(gossipsub::Event::Message {
            message_id,
            message,
            ..
        })) => {
            let Some(peer_id) = message.source else {
                return ControlFlow::Continue(());
            };

            let Some(channel) = Channel::from_topic_hash(&message.topic) else {
                trace!(
                    "Received message {message_id} from {peer_id} on different channel: {}",
                    message.topic
                );

                return ControlFlow::Continue(());
            };

            trace!(
                "Received message {message_id} from {peer_id} on channel {} of {} bytes",
                channel,
                message.data.len()
            );

            let Ok(network_msg) = NetworkMsg::from_network_bytes(&message.data) else {
                error!("Error decoding message {message_id} from {peer_id}: invalid format");
                return ControlFlow::Continue(());
            };

            let event = Event::Message(channel, peer_id, message_id, network_msg);

            if let Err(e) = tx_event.send(event).await {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
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

        swarm_event => {
            metrics.record(&swarm_event);
        }
    }

    ControlFlow::Continue(())
}
