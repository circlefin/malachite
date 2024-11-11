use std::collections::HashSet;
use std::iter;
use std::time::Duration;

use either::Either;
use libp2p::request_response::{self, OutboundRequestId, ProtocolSupport, ResponseChannel};
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

#[derive(Debug)]
pub enum NetworkEvent {
    RequestResponse(request_response::Event<Request, Response>),
}

impl From<request_response::Event<Request, Response>> for NetworkEvent {
    fn from(event: request_response::Event<Request, Response>) -> Self {
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
    pub request_response: request_response::cbor::Behaviour<Request, Response>,
}

// pub type Event = request_response::Event<Request, Response>;
// pub type Behaviour = request_response::cbor::Behaviour<Request, Response>;

fn request_response_protocol() -> iter::Once<(StreamProtocol, ProtocolSupport)> {
    iter::once((
        StreamProtocol::new(DISCOVERY_PROTOCOL),
        ProtocolSupport::Full,
    ))
}

fn request_response_config() -> request_response::Config {
    request_response::Config::default().with_request_timeout(Duration::from_secs(5))
}

impl Behaviour {
    pub fn new() -> Self {
        let request_response = request_response::cbor::Behaviour::new(
            request_response_protocol(),
            request_response_config(),
        );

        Self { request_response }
    }
}

// pub fn new_behaviour() -> Behaviour {
//     Behaviour::new(request_response_protocol(), request_response_config())
// }

pub trait SendRequestResponse: NetworkBehaviour {
    fn send_request(&mut self, peer_id: &PeerId, req: Request) -> OutboundRequestId;

    fn send_response(
        &mut self,
        ch: ResponseChannel<Response>,
        rs: Response,
    ) -> Result<(), Response>;
}
