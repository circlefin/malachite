use alloc::vec::Vec;

use crate::{CurveConfig, PublicKey};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::base64string;

#[derive(Serialize, Deserialize)]
struct PublicKeyRepr {
    #[serde(rename = "type")]
    key_type: String,
    #[serde(with = "base64string")]
    value: Vec<u8>,
}

impl<C: CurveConfig> Serialize for PublicKey<C> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let repr = PublicKeyRepr {
            key_type: C::PUBLIC_KEY_TYPE.to_string(),
            value: self.to_vec(),
        };

        repr.serialize(serializer)
    }
}

impl<'de, C: CurveConfig> Deserialize<'de> for PublicKey<C> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let repr = PublicKeyRepr::deserialize(deserializer)?;

        if repr.key_type != C::PUBLIC_KEY_TYPE {
            return Err(serde::de::Error::custom("unexpected public key type"));
        }

        PublicKey::from_sec1_bytes(repr.value.as_slice())
            .map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}
