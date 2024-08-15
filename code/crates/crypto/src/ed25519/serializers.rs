/// Serde Ed25519 VerificationKey CometBFT serializer/deserializer.
pub mod verification_key {
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
}

/// Serde Ed25519 SigningKey CometBFT serializer/deserializer.
pub mod signing_key {
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
}

/// Serialize/deserialize between base64-encoded String and Vec<u8>
pub mod base64string {
    use base64::prelude::BASE64_STANDARD;
    use base64::Engine;
    use serde::{Deserialize, Serializer};

    pub fn serialize<S>(s: &Vec<u8>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ser.serialize_str(BASE64_STANDARD.encode(s).as_str())
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(de)?;
        BASE64_STANDARD
            .decode(s)
            .map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}
