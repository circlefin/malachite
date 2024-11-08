//! Serde Ed25519 SigningKey CometBFT serializer/deserializer.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use ed25519_consensus::SigningKey;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
struct PrivKey {
    #[serde(rename = "type")]
    key_type: String,
    #[serde(with = "super::base64string")]
    value: Vec<u8>,
}

pub fn serialize<S>(s: &SigningKey, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    PrivKey {
        key_type: "tendermint/PrivKeyEd25519".to_string(),
        value: s.as_bytes().to_vec(),
    }
    .serialize(ser)
}

pub fn deserialize<'de, D>(de: D) -> Result<SigningKey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let pk = PrivKey::deserialize(de)?;
    SigningKey::try_from(pk.value.as_slice()).map_err(serde::de::Error::custom)
}
