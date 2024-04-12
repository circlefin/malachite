/// Serde base64-encoded String serializer/deserializer
pub mod serde_base64 {
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

/// Serde comma-separated String serializer/deserializer for Vec<DebugSection>.
pub mod serde_debug_section_slice {
    use crate::logging::DebugSection;
    use clap::ValueEnum;
    use serde::{Deserialize, Serializer};

    pub fn serialize<S>(s: &[DebugSection], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let r = s
            .iter()
            .map(|s| format!("{:?}", s).to_lowercase())
            .collect::<Vec<String>>()
            .join(",");
        ser.serialize_str(r.as_str())
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Vec<DebugSection>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(de)?;
        if s.is_empty() {
            return Ok(vec![]);
        }
        s.split(',')
            .map(|s| DebugSection::from_str(s, true).map_err(serde::de::Error::custom))
            .collect()
    }
}

/// Serde String serializer/deserializer for Duration.
pub mod serde_duration {
    use serde::{Deserialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(s: &Duration, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if s.as_nanos() % 1000 != 0 {
            return ser.serialize_str(format!("{}ns", s.as_nanos()).as_str());
        }
        if s.as_micros() % 1000 != 0 {
            return ser.serialize_str(format!("{}us", s.as_micros()).as_str());
        }
        if s.as_millis() % 1000 != 0 {
            return ser.serialize_str(format!("{}ms", s.as_millis()).as_str());
        }
        return ser.serialize_str(format!("{}s", s.as_secs()).as_str());
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Duration, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(de)?;
        let s = s.trim();

        // Parse the duration as nanoseconds
        if let Some(time) = s.strip_suffix("ns") {
            let u = time.parse().map_err(serde::de::Error::custom)?;
            return Ok(Duration::from_nanos(u));
        }

        // Parse the duration as microseconds
        if let Some(time) = s.strip_suffix("us") {
            let u = time.parse().map_err(serde::de::Error::custom)?;
            return Ok(Duration::from_micros(u));
        }

        // Parse the duration as milliseconds
        if let Some(time) = s.strip_suffix("ms") {
            let u = time.parse().map_err(serde::de::Error::custom)?;
            return Ok(Duration::from_millis(u));
        }

        // Parse the duration as seconds
        if let Some(time) = s.strip_suffix('s') {
            let u = time.parse().map_err(serde::de::Error::custom)?;
            return Ok(Duration::from_secs(u));
        }

        // Fall back on parsing the duration as a number of seconds if no unit was specified
        let u = s.parse().map_err(serde::de::Error::custom)?;
        Ok(Duration::from_secs(u))
    }
}

/// Serde Ed25519 PublicKey serializer/deserializer.
pub mod serde_pubkey {
    use serde::{Deserialize, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct PubKey {
        #[serde(rename = "type")]
        key_type: String,
        #[serde(with = "crate::config::serialization::serde_base64")]
        value: Vec<u8>,
    }

    pub fn serialize<S>(s: &malachite_test::PublicKey, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PubKey {
            key_type: "tendermint/PubKeyEd25519".to_string(),
            value: s.inner().to_bytes().to_vec(),
        }
        .serialize(ser)
    }

    pub fn deserialize<'de, D>(de: D) -> Result<malachite_test::PublicKey, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let pk = PubKey::deserialize(de)?;
        // Work around malachite_test::PublicKey not able to import bytes, for now.
        let vk: ed25519_consensus::VerificationKey = pk
            .value
            .as_slice()
            .try_into()
            .map_err(serde::de::Error::custom)?;
        Ok(malachite_test::PublicKey::new(vk))
    }
}
