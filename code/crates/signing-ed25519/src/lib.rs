#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

use alloc::vec::Vec;

use malachitebft_core_types::SigningScheme;
use signature::{Keypair, Signer, Verifier};

#[cfg(feature = "rand")]
use rand::{CryptoRng, RngCore};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
#[cfg_attr(coverage_nightly, coverage(off))]
mod serializers;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ed25519;

impl Ed25519 {
    #[cfg(feature = "rand")]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
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

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PrivateKey(
    #[cfg_attr(feature = "serde", serde(with = "self::serializers::signing_key"))]
    ed25519_consensus::SigningKey,
);

impl PrivateKey {
    #[cfg(feature = "rand")]
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
    pub fn sign(&self, msg: &[u8]) -> Signature {
        Signature(self.0.sign(msg))
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct PublicKey(
    #[cfg_attr(feature = "serde", serde(with = "self::serializers::verification_key"))]
    ed25519_consensus::VerificationKey,
);

impl PublicKey {
    pub fn new(key: impl Into<ed25519_consensus::VerificationKey>) -> Self {
        Self(key.into())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(ed25519_consensus::VerificationKey::try_from(bytes).unwrap())
    }

    pub fn verify(&self, msg: &[u8], signature: &Signature) -> Result<(), signature::Error> {
        self.0
            .verify(signature.inner(), msg)
            .map_err(|_| signature::Error::new())
    }

    pub fn inner(&self) -> &ed25519_consensus::VerificationKey {
        &self.0
    }
}

impl Verifier<Signature> for PublicKey {
    fn verify(&self, msg: &[u8], signature: &Signature) -> Result<(), signature::Error> {
        PublicKey::verify(self, msg, signature)
    }
}
