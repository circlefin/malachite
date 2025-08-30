use std::time::Duration;

use eyre::Result;
pub use libp2p::identity::Keypair;
use libp2p::kad::{Addresses, KBucketKey, KBucketRef};
use libp2p::request_response::{OutboundRequestId, ResponseChannel};
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{gossipsub, identify, ping};
pub use libp2p::{Multiaddr, PeerId};
use libp2p_broadcast as broadcast;

use malachitebft_discovery as discovery;
use malachitebft_metrics::Registry;
use malachitebft_sync as sync;

use crate::{Config, GossipSubConfig};
#[derive(Debug)]
pub enum NetworkEvent {
    Identify(Box<identify::Event>),
    Ping(ping::Event),
    GossipSub(gossipsub::Event),
    Broadcast(broadcast::Event),
    Sync(sync::Event),
    Discovery(Box<discovery::NetworkEvent>),
}

impl From<identify::Event> for NetworkEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(Box::new(event))
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

impl From<sync::Event> for NetworkEvent {
    fn from(event: sync::Event) -> Self {
        Self::Sync(event)
    }
}

impl From<discovery::NetworkEvent> for NetworkEvent {
    fn from(network_event: discovery::NetworkEvent) -> Self {
        Self::Discovery(Box::new(network_event))
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NetworkEvent")]
pub struct Behaviour {
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub gossipsub: Toggle<gossipsub::Behaviour>,
    pub broadcast: Toggle<broadcast::Behaviour>,
    pub sync: Toggle<sync::Behaviour>,
    pub discovery: Toggle<discovery::Behaviour>,
}

/// Dummy implementation of Debug for Behaviour.
impl std::fmt::Debug for Behaviour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Behaviour").finish()
    }
}

impl discovery::DiscoveryClient for Behaviour {
    fn add_address(&mut self, peer: &PeerId, address: Multiaddr) -> libp2p::kad::RoutingUpdate {
        self.discovery
            .as_mut()
            .expect("Discovery behaviour should be available")
            .kademlia
            .as_mut()
            .expect("Kademlia behaviour should be available")
            .add_address(peer, address)
    }

    fn kbuckets(&mut self) -> impl Iterator<Item = KBucketRef<'_, KBucketKey<PeerId>, Addresses>> {
        self.discovery
            .as_mut()
            .expect("Discovery behaviour should be available")
            .kademlia
            .as_mut()
            .expect("Kademlia behaviour should be available")
            .kbuckets()
    }

    fn send_request(&mut self, peer_id: &PeerId, req: discovery::Request) -> OutboundRequestId {
        self.discovery
            .as_mut()
            .expect("Discovery behaviour should be available")
            .request_response
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
            .request_response
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
    pub fn new_with_metrics(
        config: &Config,
        keypair: &Keypair,
        registry: &mut Registry,
    ) -> Result<Self> {
        let identify = identify::Behaviour::new(identify::Config::new(
            config.protocol_names.consensus.clone(),
            keypair.public(),
        ));

        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(5)));

        let gossipsub = config.pubsub_protocol.is_gossipsub().then(|| {
            gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                gossipsub_config(config.gossipsub, config.pubsub_max_size),
            )
            .unwrap()
            .with_metrics(
                registry.sub_registry_with_prefix("gossipsub"),
                Default::default(),
            )
        });

        let enable_broadcast = config.pubsub_protocol.is_broadcast() || config.enable_sync;
        let broadcast = enable_broadcast.then(|| {
            broadcast::Behaviour::new_with_metrics(
                broadcast::Config {
                    max_buf_size: config.pubsub_max_size,
                },
                registry.sub_registry_with_prefix("broadcast"),
            )
        });

        let sync = if config.enable_sync {
            Some(sync::Behaviour::new_with_metrics(
                sync::Config::default().with_max_response_size(config.rpc_max_size),
                config.protocol_names.sync.clone(),
                registry.sub_registry_with_prefix("sync"),
            )?)
        } else {
            None
        };

        let discovery = if config.discovery.enabled {
            Some(discovery::Behaviour::new(
                keypair,
                config.discovery,
                config.protocol_names.discovery_kad.clone(),
                config.protocol_names.discovery_regres.clone(),
            )?)
        } else {
            None
        };

        Ok(Self {
            identify,
            ping,
            sync: Toggle::from(sync),
            gossipsub: Toggle::from(gossipsub),
            broadcast: Toggle::from(broadcast),
            discovery: Toggle::from(discovery),
        })
    }
}
