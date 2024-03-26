use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use malachite_network::PeerId;
use malachite_test::PublicKey;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub peers: Vec<PeerConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerConfig {
    pub id: PeerId,
    pub addr: SocketAddr,
    #[serde(with = "de::public_key")]
    pub public_key: PublicKey,
}

pub mod de {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub mod public_key {
        use super::*;

        use malachite_test::PublicKey;

        pub fn serialize<S>(key: &PublicKey, s: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            key.inner().serialize(s)
        }

        pub fn deserialize<'de, D>(d: D) -> Result<PublicKey, D::Error>
        where
            D: Deserializer<'de>,
        {
            ed25519_consensus::VerificationKey::deserialize(d).map(PublicKey::new)
        }
    }
}
