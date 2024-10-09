use libp2p::metrics::Registry;
use libp2p::request_response::{self as rpc, ProtocolSupport};
use libp2p::swarm::NetworkBehaviour;
use libp2p::StreamProtocol;

use crate::{RawRequest, RawResponse};

// use crate::Metrics;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "Event")]
pub struct Behaviour {
    rpc: rpc::cbor::Behaviour<RawRequest, RawResponse>,
}

pub type Event = rpc::Event<RawRequest, RawResponse>;

impl Behaviour {
    pub const PROTOCOL: [(StreamProtocol, ProtocolSupport); 1] = [(
        StreamProtocol::new("/malachite-blocksync/v1beta1"),
        ProtocolSupport::Full,
    )];

    pub fn new() -> Self {
        let config = rpc::Config::default();
        Self {
            rpc: rpc::cbor::Behaviour::new(Self::PROTOCOL, config),
            // metrics: None,
        }
    }

    pub fn new_with_metrics(_registry: &mut Registry) -> Self {
        let config = rpc::Config::default();
        Self {
            rpc: rpc::cbor::Behaviour::new(Self::PROTOCOL, config),
            // metrics: Some(Metrics::new(registry)),
        }
    }
}

impl Default for Behaviour {
    fn default() -> Self {
        Self::new()
    }
}
