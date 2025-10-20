use alloc::string::ToString;
use alloc::vec::Vec;

use crate::{CurveConfig, Signature};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::base64string;

impl<C: CurveConfig> Serialize for Signature<C> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = self.to_vec();
        base64string::serialize(encoded.as_slice(), serializer)
    }
}

impl<'de, C: CurveConfig> Deserialize<'de> for Signature<C> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = base64string::deserialize(deserializer)?;
        Signature::from_slice(bytes.as_slice()).map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}
