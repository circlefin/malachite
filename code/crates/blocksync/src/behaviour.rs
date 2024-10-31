use bytes::Bytes;
use displaydoc::Display;
use libp2p::metrics::Registry;
use libp2p::request_response::{self as rpc, OutboundRequestId, ProtocolSupport};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{PeerId, StreamProtocol};

use crate::types::{RawRequest, RawResponse, ResponseChannel};

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

    pub fn send_response(&mut self, channel: ResponseChannel, data: Bytes) -> Result<(), Error> {
        self.rpc
            .send_response(channel, RawResponse(data))
            .map_err(|_| Error::SendResponse)
    }

    pub fn send_request(&mut self, peer: PeerId, data: Bytes) -> OutboundRequestId {
        self.rpc.send_request(&peer, RawRequest(data))
    }
}

#[derive(Clone, Debug, Display)]
pub enum Error {
    #[displaydoc("Failed to send response")]
    SendResponse,

    #[displaydoc("Failed to send request")]
    SendRequest,
}

impl core::error::Error for Error {}

impl Default for Behaviour {
    fn default() -> Self {
        Self::new()
    }
}
