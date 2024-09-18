use std::collections::HashSet;

use libp2p::{request_response, swarm::behaviour::toggle::Toggle, Multiaddr, PeerId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Request {
    Peers,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Response {
    Peers(HashSet<(PeerId, Multiaddr)>),
}

pub type ReqResEvent = request_response::Event<Request, Response>;
pub type ReqResBehaviour = request_response::cbor::Behaviour<Request, Response>;
pub type ToggleReqResBehaviour = Toggle<ReqResBehaviour>;
