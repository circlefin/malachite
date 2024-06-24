use libp2p::swarm::NetworkBehaviour;
use libp2p::{gossipsub, identify};

pub use libp2p::identity::Keypair;
pub use libp2p::{Multiaddr, PeerId};
use malachite_metrics::Registry;

use crate::PROTOCOL_VERSION;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NetworkEvent")]
pub struct Behaviour {
    pub identify: identify::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
}

impl Behaviour {
    pub fn new(keypair: &Keypair) -> Self {
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

    pub fn new_with_metrics(keypair: &Keypair, registry: &mut Registry) -> Self {
        Self {
            identify: identify::Behaviour::new(identify::Config::new(
                PROTOCOL_VERSION.to_string(),
                keypair.public(),
            )),
            gossipsub: gossipsub::Behaviour::new_with_metrics(
                gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                gossipsub::Config::default(),
                registry,
                Default::default(),
            )
            .unwrap(),
        }
    }
}

#[derive(Debug)]
pub enum NetworkEvent {
    Identify(identify::Event),
    GossipSub(gossipsub::Event),
}

impl From<identify::Event> for NetworkEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

impl From<gossipsub::Event> for NetworkEvent {
    fn from(event: gossipsub::Event) -> Self {
        Self::GossipSub(event)
    }
}
