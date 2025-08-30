use std::collections::HashSet;
use std::iter;
use std::time::Duration;

use either::Either;
use eyre::Result;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Request {
    Peers(HashSet<(Option<PeerId>, Vec<Multiaddr>)>),
    Connect(),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Response {
    Peers(HashSet<(Option<PeerId>, Vec<Multiaddr>)>),
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

fn kademlia_config(name: String) -> Result<kad::Config> {
    let mut config = kad::Config::new(StreamProtocol::try_from_owned(name)?);

    // In production, one might set this to a high value to keep a fresh view of the network
    config.set_periodic_bootstrap_interval(None);

    Ok(config)
}

fn request_response_protocol(
    protocol_name: String,
) -> Result<iter::Once<(StreamProtocol, ProtocolSupport)>> {
    Ok(iter::once((
        StreamProtocol::try_from_owned(protocol_name)?,
        ProtocolSupport::Full,
    )))
}

fn request_response_config() -> request_response::Config {
    request_response::Config::default().with_request_timeout(Duration::from_secs(5))
}

impl Behaviour {
    pub fn new(
        keypair: &Keypair,
        config: Config,
        discovery_kad_protocol: String,
        discovery_regres_protocol: String,
    ) -> Result<Self> {
        Self::new_with_protocols(
            keypair,
            config,
            discovery_kad_protocol,
            discovery_regres_protocol,
        )
    }

    pub fn new_with_protocols(
        keypair: &Keypair,
        config: Config,
        discovery_kad_protocol: String,
        discovery_regres_protocol: String,
    ) -> Result<Self> {
        let kademlia_config = kademlia_config(discovery_kad_protocol)?;
        let kademlia = Toggle::from(
            (config.enabled && config.bootstrap_protocol == BootstrapProtocol::Kademlia).then(
                || {
                    let mut kademlia = kad::Behaviour::with_config(
                        keypair.public().to_peer_id(),
                        MemoryStore::new(keypair.public().to_peer_id()),
                        kademlia_config,
                    );

                    kademlia.set_mode(Some(Mode::Server));

                    kademlia
                },
            ),
        );

        let request_response = request_response::cbor::Behaviour::new(
            request_response_protocol(discovery_regres_protocol)?,
            request_response_config(),
        );

        Ok(Self {
            kademlia,
            request_response,
        })
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
