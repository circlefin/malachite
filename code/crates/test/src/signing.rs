use malachite_common::SigningScheme;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use signature::{Keypair, Signer, Verifier};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ed25519;

impl Ed25519 {
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn generate_keypair<R>(rng: R) -> PrivateKey
    where
        R: RngCore + CryptoRng,
    {
        PrivateKey::generate(rng)
    }
}

impl SigningScheme for Ed25519 {
    type DecodingError = ed25519_consensus::Error;

    type Signature = Signature;
    type PublicKey = PublicKey;
    type PrivateKey = PrivateKey;

    fn encode_signature(signature: &Signature) -> Vec<u8> {
        signature.to_bytes().to_vec()
    }

    fn decode_signature(bytes: &[u8]) -> Result<Self::Signature, Self::DecodingError> {
        Signature::try_from(bytes)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Signature(ed25519_consensus::Signature);

impl Signature {
    pub fn inner(&self) -> &ed25519_consensus::Signature {
        &self.0
    }

    pub fn to_bytes(&self) -> [u8; 64] {
        self.0.to_bytes()
    }

    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(ed25519_consensus::Signature::from(bytes))
    }

    pub fn test() -> Signature {
        Signature(ed25519_consensus::Signature::from([0; 64]))
    }
}

impl From<ed25519_consensus::Signature> for Signature {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from(signature: ed25519_consensus::Signature) -> Self {
        Self(signature)
    }
}

impl TryFrom<&[u8]> for Signature {
    type Error = ed25519_consensus::Error;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self(ed25519_consensus::Signature::try_from(bytes)?))
    }
}

impl PartialOrd for Signature {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Signature {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.to_bytes().cmp(&other.0.to_bytes())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrivateKey(
    #[serde(with = "self::serializers::signing_key")] ed25519_consensus::SigningKey,
);

impl PrivateKey {
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn generate<R>(rng: R) -> Self
    where
        R: RngCore + CryptoRng,
    {
        let signing_key = ed25519_consensus::SigningKey::new(rng);

        Self(signing_key)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn public_key(&self) -> PublicKey {
        PublicKey::new(self.0.verification_key())
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn inner(&self) -> &ed25519_consensus::SigningKey {
        &self.0
    }
}

impl From<[u8; 32]> for PrivateKey {
    fn from(bytes: [u8; 32]) -> Self {
        Self(ed25519_consensus::SigningKey::from(bytes))
    }
}

impl Signer<Signature> for PrivateKey {
    fn try_sign(&self, msg: &[u8]) -> Result<Signature, signature::Error> {
        Ok(Signature(self.0.sign(msg)))
    }
}

impl Keypair for PrivateKey {
    type VerifyingKey = PublicKey;

    fn verifying_key(&self) -> Self::VerifyingKey {
        self.public_key()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PublicKey(
    #[serde(with = "self::serializers::verification_key")] ed25519_consensus::VerificationKey,
);

impl PublicKey {
    pub fn new(key: impl Into<ed25519_consensus::VerificationKey>) -> Self {
        Self(key.into())
    }

    pub fn hash(&self) -> [u8; 32] {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(self.0.as_bytes());
        hasher.finalize().into()
    }

    pub fn inner(&self) -> &ed25519_consensus::VerificationKey {
        &self.0
    }
}

impl Verifier<Signature> for PublicKey {
    fn verify(&self, msg: &[u8], signature: &Signature) -> Result<(), signature::Error> {
        self.0
            .verify(signature.inner(), msg)
            .map_err(|_| signature::Error::new())
    }
}

pub mod serializers {
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
}
