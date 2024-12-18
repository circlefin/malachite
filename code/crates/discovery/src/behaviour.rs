use std::collections::HashSet;
use std::iter;
use std::time::Duration;

use either::Either;
use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{Addresses, KBucketKey, KBucketRef, Mode, RoutingUpdate};
use libp2p::request_response::{self, OutboundRequestId, ProtocolSupport, ResponseChannel};
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{kad, Multiaddr, PeerId, StreamProtocol};
use serde::{Deserialize, Serialize};

use crate::config::BootstrapProtocol;
use crate::Config;

const DISCOVERY_KAD_PROTOCOL: &str = "/malachitebft-discovery/kad/v1beta1";
const DISCOVERY_REQRES_PROTOCOL: &str = "/malachitebft-discovery/reqres/v1beta1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Request {
    Peers(HashSet<(Option<PeerId>, Multiaddr)>),
    Connect(),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Response {
    Peers(HashSet<(Option<PeerId>, Multiaddr)>),
    Connect(bool),
}

#[derive(Debug)]
pub enum NetworkEvent {
    Kademlia(kad::Event),
    RequestResponse(request_response::Event<Request, Response>),
}

impl From<kad::Event> for NetworkEvent {
    fn from(event: kad::Event) -> Self {
        Self::Kademlia(event)
    }
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
    pub kademlia: Toggle<kad::Behaviour<MemoryStore>>,
    pub request_response: request_response::cbor::Behaviour<Request, Response>,
}

fn kademlia_config() -> kad::Config {
    let mut config = kad::Config::new(StreamProtocol::new(DISCOVERY_KAD_PROTOCOL));

    // In production, one might set this to a high value to keep a fresh view of the network
    config.set_periodic_bootstrap_interval(None);

    config
}

fn request_response_protocol() -> iter::Once<(StreamProtocol, ProtocolSupport)> {
    iter::once((
        StreamProtocol::new(DISCOVERY_REQRES_PROTOCOL),
        ProtocolSupport::Full,
    ))
}

fn request_response_config() -> request_response::Config {
    request_response::Config::default().with_request_timeout(Duration::from_secs(5))
}

impl Behaviour {
    pub fn new(keypair: &Keypair, config: Config) -> Self {
        let kademlia = Toggle::from(
            (config.enabled && config.bootstrap_protocol == BootstrapProtocol::Kademlia).then(
                || {
                    let mut kademlia = kad::Behaviour::with_config(
                        keypair.public().to_peer_id(),
                        MemoryStore::new(keypair.public().to_peer_id()),
                        kademlia_config(),
                    );

                    kademlia.set_mode(Some(Mode::Server));

                    kademlia
                },
            ),
        );

        let request_response = request_response::cbor::Behaviour::new(
            request_response_protocol(),
            request_response_config(),
        );

        Self {
            kademlia,
            request_response,
        }
    }
}

pub trait DiscoveryClient: NetworkBehaviour {
    fn add_address(&mut self, peer: &PeerId, address: Multiaddr) -> RoutingUpdate;

    fn kbuckets(&mut self) -> impl Iterator<Item = KBucketRef<'_, KBucketKey<PeerId>, Addresses>>;

    fn send_request(&mut self, peer_id: &PeerId, req: Request) -> OutboundRequestId;

    fn send_response(
        &mut self,
        ch: ResponseChannel<Response>,
        rs: Response,
    ) -> Result<(), Response>;
}
