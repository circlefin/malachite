use alloc::string::{String, ToString};
use alloc::vec::Vec;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serializer};

pub fn serialize<S>(s: &[u8], ser: S) -> Result<S::Ok, S::Error>
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
