//! Serde Ed25519 VerificationKey CometBFT serializer/deserializer.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use ed25519_consensus::VerificationKey;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
struct PubKey {
    #[serde(rename = "type")]
    key_type: String,
    #[serde(with = "super::base64string")]
    value: Vec<u8>,
}

pub fn serialize<S>(s: &VerificationKey, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    PubKey {
        key_type: "tendermint/PubKeyEd25519".to_string(),
        value: s.as_bytes().to_vec(),
    }
    .serialize(ser)
}

pub fn deserialize<'de, D>(de: D) -> Result<VerificationKey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let pk = PubKey::deserialize(de)?;
    VerificationKey::try_from(pk.value.as_slice()).map_err(serde::de::Error::custom)
}
