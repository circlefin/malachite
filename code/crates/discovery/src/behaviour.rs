use std::collections::HashSet;
use std::iter;
use std::time::Duration;

use libp2p::request_response::{self, ProtocolSupport, ResponseChannel};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{Multiaddr, PeerId, StreamProtocol};
use serde::{Deserialize, Serialize};

use crate::DISCOVERY_PROTOCOL;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Request {
    Peers(HashSet<(Option<PeerId>, Multiaddr)>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Response {
    Peers(HashSet<(Option<PeerId>, Multiaddr)>),
}

pub type Event = request_response::Event<Request, Response>;
pub type Behaviour = request_response::cbor::Behaviour<Request, Response>;

fn request_response_protocol() -> iter::Once<(StreamProtocol, ProtocolSupport)> {
    iter::once((
        StreamProtocol::new(DISCOVERY_PROTOCOL),
        ProtocolSupport::Full,
    ))
}

fn request_response_config() -> request_response::Config {
    request_response::Config::default().with_request_timeout(Duration::from_secs(5))
}

pub fn new_behaviour() -> Behaviour {
    Behaviour::new(request_response_protocol(), request_response_config())
}

pub trait SendResponse: NetworkBehaviour {
    fn send_response(
        &mut self,
        ch: ResponseChannel<Response>,
        rs: Response,
    ) -> Result<(), Response>;
}
