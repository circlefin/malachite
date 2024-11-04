use std::time::Duration;

use either::Either;
use libp2p::request_response::{OutboundRequestId, ResponseChannel};
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{gossipsub, identify, ping};
use libp2p_broadcast as broadcast;

pub use libp2p::identity::Keypair;
pub use libp2p::{Multiaddr, PeerId};

use malachite_blocksync as blocksync;
use malachite_discovery as discovery;
use malachite_metrics::Registry;

use crate::{Config, GossipSubConfig, PubSubProtocol, PROTOCOL};

#[derive(Debug)]
pub enum NetworkEvent {
    Identify(identify::Event),
    Ping(ping::Event),
    GossipSub(gossipsub::Event),
    Broadcast(broadcast::Event),
    BlockSync(blocksync::Event),
    RequestResponse(discovery::Event),
}

impl From<identify::Event> for NetworkEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

impl From<ping::Event> for NetworkEvent {
    fn from(event: ping::Event) -> Self {
        Self::Ping(event)
    }
}

impl From<gossipsub::Event> for NetworkEvent {
    fn from(event: gossipsub::Event) -> Self {
        Self::GossipSub(event)
    }
}

impl From<broadcast::Event> for NetworkEvent {
    fn from(event: broadcast::Event) -> Self {
        Self::Broadcast(event)
    }
}

impl From<blocksync::Event> for NetworkEvent {
    fn from(event: blocksync::Event) -> Self {
        Self::BlockSync(event)
    }
}

impl From<discovery::Event> for NetworkEvent {
    fn from(event: discovery::Event) -> Self {
        Self::RequestResponse(event)
    }
}

impl<A, B> From<Either<A, B>> for NetworkEvent
where
    A: Into<NetworkEvent>,
    B: Into<NetworkEvent>,
{
    fn from(event: Either<A, B>) -> Self {
        match event {
            Either::Left(event) => event.into(),
            Either::Right(event) => event.into(),
        }
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NetworkEvent")]
pub struct Behaviour {
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub pubsub: Either<gossipsub::Behaviour, broadcast::Behaviour>,
    pub blocksync: blocksync::Behaviour,
    pub discovery: Toggle<discovery::Behaviour>,
}

impl discovery::SendRequestResponse for Behaviour {
    fn send_request(&mut self, peer_id: &PeerId, req: discovery::Request) -> OutboundRequestId {
        self.discovery
            .as_mut()
            .expect("Discovery behaviour should be available")
            .send_request(peer_id, req)
    }

    fn send_response(
        &mut self,
        ch: ResponseChannel<discovery::Response>,
        rs: discovery::Response,
    ) -> Result<(), discovery::Response> {
        self.discovery
            .as_mut()
            .expect("Discovery behaviour should be available")
            .send_response(ch, rs)
    }
}

fn message_id(message: &gossipsub::Message) -> gossipsub::MessageId {
    use seahash::SeaHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = SeaHasher::new();
    message.hash(&mut hasher);
    gossipsub::MessageId::new(hasher.finish().to_be_bytes().as_slice())
}

fn gossipsub_config(config: GossipSubConfig, max_transmit_size: usize) -> gossipsub::Config {
    gossipsub::ConfigBuilder::default()
        .max_transmit_size(max_transmit_size)
        .opportunistic_graft_ticks(3)
        .heartbeat_interval(Duration::from_secs(1))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .history_gossip(3)
        .history_length(5)
        .mesh_n_high(config.mesh_n_high)
        .mesh_n_low(config.mesh_n_low)
        .mesh_outbound_min(config.mesh_outbound_min)
        .mesh_n(config.mesh_n)
        .message_id_fn(message_id)
        .build()
        .unwrap()
}

impl Behaviour {
    pub fn new_with_metrics(config: &Config, keypair: &Keypair, registry: &mut Registry) -> Self {
        let identify = identify::Behaviour::new(identify::Config::new(
            PROTOCOL.to_string(),
            keypair.public(),
        ));

        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(5)));

        let pubsub = match config.protocol {
            PubSubProtocol::GossipSub(cfg) => Either::Left(
                gossipsub::Behaviour::new_with_metrics(
                    gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                    gossipsub_config(cfg, config.pubsub_max_size),
                    registry.sub_registry_with_prefix("gossipsub"),
                    Default::default(),
                )
                .unwrap(),
            ),
            PubSubProtocol::Broadcast => Either::Right(broadcast::Behaviour::new_with_metrics(
                broadcast::Config {
                    max_buf_size: config.pubsub_max_size,
                },
                registry.sub_registry_with_prefix("broadcast"),
            )),
        };

        let blocksync = blocksync::Behaviour::new_with_metrics(
            blocksync::Config::default().with_max_response_size(config.rpc_max_size),
            registry.sub_registry_with_prefix("blocksync"),
        );

        let discovery = Toggle::from(config.discovery.enabled.then(discovery::new_behaviour));

        Self {
            identify,
            ping,
            pubsub,
            blocksync,
            discovery,
        }
    }
}
