use malachite_common::SigningScheme;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use signature::{Keypair, Signer, Verifier};

pub use ed25519_consensus::Signature;

pub mod serializers;

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
        Ok(self.0.sign(msg))
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
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
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
            .verify(signature, msg)
            .map_err(|_| signature::Error::new())
    }
}
