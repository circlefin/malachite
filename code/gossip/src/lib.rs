use std::error::Error;
use std::ops::ControlFlow;
use std::time::Duration;

use futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::swarm::{self, NetworkBehaviour, SwarmEvent};
use libp2p::{gossipsub, identify, noise, tcp, yamux, Multiaddr, PeerId, SwarmBuilder};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

const PROTOCOL_VERSION: &str = "malachite-gossip/v1beta1";
const TOPIC: &str = "consensus";

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "Event")]
struct Behaviour {
    identify: identify::Behaviour,
    gossipsub: gossipsub::Behaviour,
}

impl Behaviour {
    fn new(keypair: &Keypair) -> Self {
        Self {
            identify: identify::Behaviour::new(identify::Config::new(
                PROTOCOL_VERSION.to_string(),
                keypair.public(),
            )),
            gossipsub: gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                gossipsub::Config::default(),
            )
            .unwrap(),
        }
    }
}

#[derive(Debug)]
enum Event {
    Identify(identify::Event),
    GossipSub(gossipsub::Event),
}

impl From<identify::Event> for Event {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

impl From<gossipsub::Event> for Event {
    fn from(event: gossipsub::Event) -> Self {
        Self::GossipSub(event)
    }
}

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
pub enum HandleEvent {
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

pub struct Handle {
    rx_event: mpsc::Receiver<HandleEvent>,
    tx_ctrl: mpsc::Sender<CtrlMsg>,
    task_handle: tokio::task::JoinHandle<()>,
}

impl Handle {
    pub async fn recv(&mut self) -> Option<HandleEvent> {
        self.rx_event.recv().await
    }

    pub async fn broadcast(&mut self, data: Vec<u8>) -> Result<(), Box<dyn Error>> {
        self.tx_ctrl.send(CtrlMsg::Broadcast(data)).await?;
        Ok(())
    }

    pub async fn shutdown(self) -> Result<(), Box<dyn Error>> {
        self.tx_ctrl.send(CtrlMsg::Shutdown).await?;
        self.task_handle.await?;
        Ok(())
    }
}

pub async fn spawn(
    keypair: Keypair,
    addr: Multiaddr,
    config: Config,
) -> Result<Handle, Box<dyn Error>> {
    let mut swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(Behaviour::new)?
        .with_swarm_config(|cfg| config.apply(cfg))
        .build();

    let topic = gossipsub::IdentTopic::new(TOPIC);
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    swarm.listen_on(addr)?;

    let (tx_event, rx_event) = mpsc::channel(32);
    let (tx_ctrl, rx_ctrl) = mpsc::channel(32);
    let task_handle = tokio::task::spawn(run(swarm, topic, rx_ctrl, tx_event));

    Ok(Handle {
        rx_event,
        tx_ctrl,
        task_handle,
    })
}

async fn run(
    mut swarm: swarm::Swarm<Behaviour>,
    topic: gossipsub::IdentTopic,
    mut rx_ctrl: mpsc::Receiver<CtrlMsg>,
    tx_event: mpsc::Sender<HandleEvent>,
) {
    loop {
        let result = tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &topic, &tx_event).await
            }

            Some(ctrl) = rx_ctrl.recv() => {
                handle_ctrl_msg(ctrl, &topic, &mut swarm).await
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
    topic: &gossipsub::IdentTopic,
    swarm: &mut swarm::Swarm<Behaviour>,
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
    event: SwarmEvent<Event>,
    topic: &gossipsub::IdentTopic,
    tx_event: &mpsc::Sender<HandleEvent>,
) -> ControlFlow<()> {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            info!("Node is listening on {address}");

            if let Err(e) = tx_event.send(HandleEvent::Listening(address)).await {
                error!("Error sending listening event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        SwarmEvent::Behaviour(Event::Identify(identify::Event::Sent { peer_id })) => {
            debug!("Sent identity to {peer_id}");
        }

        SwarmEvent::Behaviour(Event::Identify(identify::Event::Received { peer_id, info: _ })) => {
            debug!("Received identity from {peer_id}");
        }

        SwarmEvent::Behaviour(Event::GossipSub(gossipsub::Event::Subscribed {
            peer_id,
            topic: topic_hash,
        })) => {
            if topic.hash() != topic_hash {
                debug!("Peer {peer_id} subscribed to different topic: {topic_hash}");
                return ControlFlow::Continue(());
            }

            debug!("Peer {peer_id} subscribed to {topic_hash}");

            if let Err(e) = tx_event.send(HandleEvent::PeerConnected(peer_id)).await {
                error!("Error sending peer connected event to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        SwarmEvent::Behaviour(Event::GossipSub(gossipsub::Event::Message {
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

            if let Err(e) = tx_event
                .send(HandleEvent::Message(peer_id, message.data))
                .await
            {
                error!("Error sending message to handle: {e}");
                return ControlFlow::Break(());
            }
        }

        _ => {}
    }

    ControlFlow::Continue(())
}
