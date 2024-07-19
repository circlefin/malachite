// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use core::fmt;
use std::collections::HashMap;
use std::error::Error;
use std::ops::ControlFlow;
use std::time::Duration;

use futures::StreamExt;
use libp2p::swarm::{self, SwarmEvent};
use libp2p::{gossipsub, identify, SwarmBuilder};
use libp2p_tls as _; // https://github.com/informalsystems/malachite/issues/269
use tokio::sync::mpsc;
use tracing::{debug, error, error_span, trace, Instrument};

use malachite_metrics::SharedRegistry;

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

const PROTOCOL_VERSION: &str = "malachite-gossip-mempool/v1beta1";

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

#[derive(Debug, Default)]
pub struct State {
    pub peers: HashMap<PeerId, identify::Info>,
}

#[derive(Debug)]
pub enum Event {
    Listening(Multiaddr),
    Message(PeerId, NetworkMsg),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

#[derive(Debug)]
pub enum CtrlMsg {
    Broadcast(Channel, Vec<u8>),
    Shutdown,
}

pub async fn spawn(
    keypair: Keypair,
    config: Config,
    registry: SharedRegistry,
) -> Result<Handle, BoxError> {
    let mut swarm = registry.with_prefix(
        "malachite_gossip_mempool",
        |registry| -> Result<_, BoxError> {
            Ok(SwarmBuilder::with_existing_identity(keypair)
                .with_tokio()
                .with_quic()
                .with_dns()?
                .with_bandwidth_metrics(registry)
                .with_behaviour(|kp| Behaviour::new_with_metrics(kp, registry))?
                .with_swarm_config(|cfg| config.apply(cfg))
                .build())
        },
    )?;

    for channel in Channel::all() {
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&channel.to_topic())?;
    }

    let (tx_event, rx_event) = mpsc::channel(32);
    let (tx_ctrl, rx_ctrl) = mpsc::channel(32);

    let peer_id = swarm.local_peer_id();
    let span = error_span!("gossip-mempool", peer = %peer_id);
    let task_handle = tokio::task::spawn(run(config, swarm, rx_ctrl, tx_event).instrument(span));

    Ok(Handle::new(tx_ctrl, rx_event, task_handle))
}

async fn run(
    config: Config,
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
                handle_swarm_event(event, &mut swarm, &mut state, &tx_event).await
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
        CtrlMsg::Broadcast(channel, data) => {
            let msg_size = data.len();

            let result = swarm
                .behaviour_mut()
                .gossipsub
                .publish(channel.topic_hash(), data);

            match result {
                Ok(message_id) => {
                    trace!("Broadcasted message {message_id} of {msg_size} bytes");
                }
                Err(e) => {
                    error!("Error broadcasting message: {e}");
                }
            }

            ControlFlow::Continue(())
        }

        CtrlMsg::Shutdown => ControlFlow::Break(()),
    }
}

async fn handle_swarm_event(
    event: SwarmEvent<NetworkEvent>,
    swarm: &mut swarm::Swarm<Behaviour>,
    state: &mut State,
    tx_event: &mpsc::Sender<Event>,
) -> ControlFlow<()> {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            debug!("Node is listening on {address}");

            if let Err(e) = tx_event.send(Event::Listening(address)).await {
                error!("Error sending listening event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        SwarmEvent::Behaviour(NetworkEvent::Identify(identify::Event::Sent { peer_id })) => {
            trace!("Sent identity to {peer_id}");
        }

        SwarmEvent::Behaviour(NetworkEvent::Identify(identify::Event::Received {
            peer_id,
            info,
        })) => {
            trace!(
                "Received identity from {peer_id}: protocol={:?}",
                info.protocol_version
            );

            if info.protocol_version == PROTOCOL_VERSION {
                trace!(
                    "Connecting to peer {peer_id} using protocol {:?}",
                    info.protocol_version
                );

                state.peers.insert(peer_id, info);

                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
            } else {
                trace!(
                    "Peer {peer_id} is using incompatible protocol version: {:?}",
                    info.protocol_version
                );
            }
        }

        SwarmEvent::Behaviour(NetworkEvent::GossipSub(gossipsub::Event::Subscribed {
            peer_id,
            topic: topic_hash,
        })) => {
            if !Channel::has_topic(&topic_hash) {
                trace!("Peer {peer_id} tried to subscribe to unknown topic: {topic_hash}");

                return ControlFlow::Continue(());
            }

            trace!("Peer {peer_id} subscribed to {topic_hash}");

            if let Err(e) = tx_event.send(Event::PeerConnected(peer_id)).await {
                error!("Error sending peer connected event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        SwarmEvent::Behaviour(NetworkEvent::GossipSub(gossipsub::Event::Message {
            propagation_source: peer_id,
            message_id,
            message,
        })) => {
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

            if let Err(e) = tx_event.send(Event::Message(peer_id, network_msg)).await {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        _ => {}
    }

    ControlFlow::Continue(())
}
