use std::net::SocketAddr;

use derive_where::derive_where;
use malachite_common::{Context, PublicKey};
use malachite_test::TestContext;

use crate::config::{Config, PeerConfig};
use crate::network::broadcast::PeerInfo;
use crate::network::PeerId;

#[derive_where(Clone, Debug)]
pub struct Peers<Ctx: Context> {
    pub peers: Vec<Peer<Ctx>>,
}

impl<Ctx: Context> Peers<Ctx> {
    pub fn get(&self, id: &PeerId) -> Option<&Peer<Ctx>> {
        self.peers.iter().find(|p| &p.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Peer<Ctx>> {
        self.peers.iter()
    }

    pub fn except<'a>(&'a self, id: &'a PeerId) -> impl Iterator<Item = &Peer<Ctx>> + 'a {
        self.iter().filter(move |p| &p.id != id)
    }
}

impl From<Config> for Peers<TestContext> {
    fn from(config: Config) -> Self {
        Self {
            peers: config.peers.into_iter().map(Peer::from).collect(),
        }
    }
}

#[derive_where(Clone, Debug)]
pub struct Peer<Ctx: Context> {
    pub id: PeerId,
    pub addr: SocketAddr,
    pub public_key: PublicKey<Ctx>,
}

impl From<PeerConfig> for Peer<TestContext> {
    fn from(peer: PeerConfig) -> Self {
        Self {
            id: peer.id,
            addr: peer.addr,
            public_key: peer.public_key,
        }
    }
}

impl Peer<TestContext> {
    pub fn peer_info(&self) -> PeerInfo {
        PeerInfo {
            id: self.id.clone(),
            addr: self.addr,
        }
    }
}
