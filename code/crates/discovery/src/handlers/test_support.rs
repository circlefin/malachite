use libp2p::kad::{Addresses, KBucketKey, KBucketRef, RoutingUpdate};
use libp2p::request_response::{OutboundRequestId, ResponseChannel};
use libp2p::swarm::dummy;
use libp2p::{Multiaddr, PeerId};
use malachitebft_metrics::Registry;

use crate::{config::Config, Discovery, DiscoveryClient, Request, Response};

impl DiscoveryClient for dummy::Behaviour {
    fn add_address(&mut self, _peer: &PeerId, _address: Multiaddr) -> RoutingUpdate {
        unreachable!()
    }

    fn kbuckets(&mut self) -> impl Iterator<Item = KBucketRef<'_, KBucketKey<PeerId>, Addresses>> {
        std::iter::empty()
    }

    fn send_request(&mut self, _peer_id: &PeerId, _req: Request) -> OutboundRequestId {
        unreachable!()
    }

    fn send_response(
        &mut self,
        _ch: ResponseChannel<Response>,
        _rs: Response,
    ) -> Result<(), Response> {
        unreachable!()
    }
}

pub fn discovery(config: Config) -> Discovery<dummy::Behaviour> {
    let mut registry = Registry::default();
    Discovery::new(config, vec![], &mut registry)
}

pub fn discovery_with_inbound_capacity(num_inbound_peers: usize) -> Discovery<dummy::Behaviour> {
    let mut config = Config::new(false);
    config.set_peers_bounds(1, num_inbound_peers);
    discovery(config)
}
