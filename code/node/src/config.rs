use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use malachite_test::PublicKey;

use crate::network::PeerId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub peers: Vec<PeerConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerConfig {
    #[serde(with = "de::peer_id")]
    pub id: PeerId,
    pub addr: SocketAddr,
    #[serde(with = "de::public_key")]
    pub public_key: PublicKey,
}

pub mod de {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub mod peer_id {
        use super::*;

        use crate::network::PeerId;

        pub fn serialize<S>(id: &PeerId, s: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            s.serialize_str(&id.to_string())
        }

        pub fn deserialize<'de, D>(d: D) -> Result<PeerId, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = String::deserialize(d)?;
            Ok(PeerId::new(s))
        }
    }

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
