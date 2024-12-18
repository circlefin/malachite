use std::time::Duration;

use libp2p::kad::{Addresses, KBucketKey, KBucketRef};
use libp2p::request_response::{OutboundRequestId, ResponseChannel};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{gossipsub, identify, ping};
use libp2p_broadcast as broadcast;

pub use libp2p::identity::Keypair;
pub use libp2p::{Multiaddr, PeerId};

use malachitebft_discovery as discovery;
use malachitebft_metrics::Registry;
use malachitebft_sync as sync;

use crate::{Config, GossipSubConfig, PROTOCOL};

#[derive(Debug)]
pub enum NetworkEvent {
    Identify(identify::Event),
    Ping(ping::Event),
    GossipSub(gossipsub::Event),
    Broadcast(broadcast::Event),
    Sync(sync::Event),
    Discovery(discovery::NetworkEvent),
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

impl From<sync::Event> for NetworkEvent {
    fn from(event: sync::Event) -> Self {
        Self::Sync(event)
    }
}

impl From<discovery::NetworkEvent> for NetworkEvent {
    fn from(network_event: discovery::NetworkEvent) -> Self {
        Self::Discovery(network_event)
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NetworkEvent")]
pub struct Behaviour {
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
    pub broadcast: broadcast::Behaviour,
    pub sync: sync::Behaviour,
    pub discovery: discovery::Behaviour,
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
            .kademlia
            .as_mut()
            .expect("Kademlia behaviour should be available")
            .add_address(peer, address)
    }

    fn kbuckets(&mut self) -> impl Iterator<Item = KBucketRef<'_, KBucketKey<PeerId>, Addresses>> {
        self.discovery
            .kademlia
            .as_mut()
            .expect("Kademlia behaviour should be available")
            .kbuckets()
    }

    fn send_request(&mut self, peer_id: &PeerId, req: discovery::Request) -> OutboundRequestId {
        self.discovery.request_response.send_request(peer_id, req)
    }

    fn send_response(
        &mut self,
        ch: ResponseChannel<discovery::Response>,
        rs: discovery::Response,
    ) -> Result<(), discovery::Response> {
        self.discovery.request_response.send_response(ch, rs)
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

        let gossipsub = gossipsub::Behaviour::new_with_metrics(
            gossipsub::MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config(config.gossipsub, config.pubsub_max_size),
            registry.sub_registry_with_prefix("gossipsub"),
            Default::default(),
        )
        .unwrap();

        let broadcast = broadcast::Behaviour::new_with_metrics(
            broadcast::Config {
                max_buf_size: config.pubsub_max_size,
            },
            registry.sub_registry_with_prefix("broadcast"),
        );

        let sync = sync::Behaviour::new_with_metrics(
            sync::Config::default().with_max_response_size(config.rpc_max_size),
            registry.sub_registry_with_prefix("sync"),
        );

        let discovery = discovery::Behaviour::new(keypair, config.discovery);

        Self {
            identify,
            ping,
            gossipsub,
            broadcast,
            sync,
            discovery,
        }
    }
}
