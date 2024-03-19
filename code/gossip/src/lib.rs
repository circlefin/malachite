use std::error::Error;
use std::ops::ControlFlow;
use std::time::Duration;

use behaviour::{Behaviour, NetworkEvent};
use futures::StreamExt;
use handle::Handle;
use libp2p::swarm::{self, SwarmEvent};
use libp2p::{gossipsub, identify, mdns, SwarmBuilder};
use tokio::sync::mpsc;
use tracing::{debug, error, error_span, Instrument};

pub use libp2p::identity::Keypair;
pub use libp2p::{Multiaddr, PeerId};

pub mod behaviour;
pub mod handle;

const PROTOCOL_VERSION: &str = "malachite-gossip/v1beta1";
const TOPIC: &str = "consensus";

pub type BoxError = Box<dyn Error + Send + Sync + 'static>;

#[derive(Clone, Debug)]
pub struct Config {
    idle_connection_timeout: Duration,
}

impl Config {
    fn apply(self, cfg: swarm::Config) -> swarm::Config {
        cfg.with_idle_connection_timeout(self.idle_connection_timeout)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            idle_connection_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
pub enum Event {
    Listening(Multiaddr),
    Message(PeerId, Vec<u8>),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

#[derive(Debug)]
pub enum CtrlMsg {
    Broadcast(Vec<u8>),
    Shutdown,
}

pub async fn spawn(keypair: Keypair, addr: Multiaddr, config: Config) -> Result<Handle, BoxError> {
    let mut swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic()
        .with_behaviour(Behaviour::new)?
        .with_swarm_config(|cfg| config.apply(cfg))
        .build();

    let topic = gossipsub::IdentTopic::new(TOPIC);
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    swarm.listen_on(addr)?;

    let (tx_event, rx_event) = mpsc::channel(32);
    let (tx_ctrl, rx_ctrl) = mpsc::channel(32);

    let peer_id = swarm.local_peer_id();
    let span = error_span!("gossip", peer = %peer_id);
    let task_handle = tokio::task::spawn(run(swarm, topic, rx_ctrl, tx_event).instrument(span));

    Ok(Handle::new(tx_ctrl, rx_event, task_handle))
}

async fn run(
    mut swarm: swarm::Swarm<Behaviour>,
    topic: gossipsub::IdentTopic,
    mut rx_ctrl: mpsc::Receiver<CtrlMsg>,
    tx_event: mpsc::Sender<Event>,
) {
    loop {
        let result = tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &mut swarm, &topic, &tx_event).await
            }

            Some(ctrl) = rx_ctrl.recv() => {
                handle_ctrl_msg(ctrl, &mut swarm, &topic).await
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
    topic: &gossipsub::IdentTopic,
) -> ControlFlow<()> {
    match msg {
        CtrlMsg::Broadcast(data) => {
            let result = swarm.behaviour_mut().gossipsub.publish(topic.hash(), data);

            match result {
                Ok(message_id) => {
                    debug!("Broadcasted message {message_id}");
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
    topic: &gossipsub::IdentTopic,
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
            debug!("Sent identity to {peer_id}");
        }

        SwarmEvent::Behaviour(NetworkEvent::Identify(identify::Event::Received {
            peer_id,
            info: _,
        })) => {
            debug!("Received identity from {peer_id}");
        }

        SwarmEvent::Behaviour(NetworkEvent::Mdns(mdns::Event::Discovered(peers))) => {
            for (peer_id, addr) in peers {
                debug!("Discovered peer {peer_id} at {addr}");
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);

                // if let Err(e) = tx_event.send(HandleEvent::PeerConnected(peer_id)).await {
                //     error!("Error sending peer connected event to handle: {e}");
                //     return ControlFlow::Break(());
                // }
            }
        }

        SwarmEvent::Behaviour(NetworkEvent::Mdns(mdns::Event::Expired(peers))) => {
            for (peer_id, _addr) in peers {
                debug!("Expired peer: {peer_id}");
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .remove_explicit_peer(&peer_id);

                //     if let Err(e) = tx_event.send(HandleEvent::PeerDisconnected(peer_id)).await {
                //         error!("Error sending peer disconnected event to handle: {e}");
                //         return ControlFlow::Break(());
                //     }
            }
        }

        SwarmEvent::Behaviour(NetworkEvent::GossipSub(gossipsub::Event::Subscribed {
            peer_id,
            topic: topic_hash,
        })) => {
            if topic.hash() != topic_hash {
                debug!("Peer {peer_id} subscribed to different topic: {topic_hash}");
                return ControlFlow::Continue(());
            }

            debug!("Peer {peer_id} subscribed to {topic_hash}");

            if let Err(e) = tx_event.send(Event::PeerConnected(peer_id)).await {
                error!("Error sending peer connected event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        SwarmEvent::Behaviour(NetworkEvent::GossipSub(gossipsub::Event::Message {
            propagation_source: peer_id,
            message_id: _,
            message,
        })) => {
            if topic.hash() != message.topic {
                debug!(
                    "Received message from {peer_id} on different topic: {}",
                    message.topic
                );

                return ControlFlow::Continue(());
            }

            debug!(
                "Received message from {peer_id} of {} bytes",
                message.data.len()
            );

            if let Err(e) = tx_event.send(Event::Message(peer_id, message.data)).await {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        _ => {}
    }

    ControlFlow::Continue(())
}
