use std::net::{Ipv4Addr, SocketAddr};

use malachite_test::Validator;

use crate::config::{Config, PeerConfig};
use crate::network::PeerId;

pub fn make_config<'a>(vs: impl Iterator<Item = &'a Validator>) -> Config {
    let peers = vs
        .enumerate()
        .map(|(i, v)| PeerConfig {
            id: PeerId::new(format!("node{}", i + 1)),
            addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 1200 + i as u16 + 1),
            public_key: v.public_key,
        })
        .collect();

    Config { peers }
}
