use std::time::Duration;

use bytes::Bytes;
use libp2p::metrics::Registry;
use libp2p::request_response::{self as rpc, OutboundRequestId, ProtocolSupport};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{PeerId, StreamProtocol};
use thiserror::Error;

use crate::rpc::Codec;
use crate::types::{RawRequest, RawResponse, ResponseChannel};

// use crate::Metrics;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "Event")]
pub struct Behaviour {
    rpc: rpc::Behaviour<Codec>,
}

pub type Event = rpc::Event<RawRequest, RawResponse>;

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub request_timeout: Duration,
    pub max_request_size: usize,
    pub max_response_size: usize,
}

impl Config {
    pub fn with_request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = request_timeout;
        self
    }

    pub fn with_max_request_size(mut self, max_request_size: usize) -> Self {
        self.max_request_size = max_request_size;
        self
    }

    pub fn with_max_response_size(mut self, max_response_size: usize) -> Self {
        self.max_response_size = max_response_size;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            max_request_size: 1024 * 1024,        // 1 MiB
            max_response_size: 512 * 1024 * 1024, // 512 MiB
        }
    }
}

impl Behaviour {
    pub const PROTOCOL: [(StreamProtocol, ProtocolSupport); 1] = [(
        StreamProtocol::new("/malachitebft-sync/v1beta1"),
        ProtocolSupport::Full,
    )];

    pub fn new(config: Config) -> Self {
        let rpc_config = rpc::Config::default().with_request_timeout(config.request_timeout);

        Self {
            rpc: rpc::Behaviour::with_codec(Codec::new(config), Self::PROTOCOL, rpc_config),
            // metrics: None,
        }
    }

    pub fn new_with_metrics(config: Config, _registry: &mut Registry) -> Self {
        let rpc_config = rpc::Config::default().with_request_timeout(config.request_timeout);

        Self {
            rpc: rpc::Behaviour::with_codec(Codec::new(config), Self::PROTOCOL, rpc_config),
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

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("Failed to send response")]
    SendResponse,

    #[error("Failed to send request")]
    SendRequest,
}

impl Default for Behaviour {
    fn default() -> Self {
        Self::new(Config::default())
    }
}
