#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt::Debug;
use core::marker::PhantomData;

use malachitebft_core_types::SigningScheme;
use signature::{Keypair, Signer, Verifier};

pub use signature::Error as SignatureError;

#[cfg(feature = "rand")]
use rand::{CryptoRng, RngCore};

#[cfg(feature = "serde")]
#[cfg_attr(coverage_nightly, coverage(off))]
pub mod serializers;

mod curve;

#[cfg(feature = "k256")]
pub use curve::K256Config;
#[cfg(feature = "p256")]
pub use curve::P256Config;
#[cfg(feature = "p384")]
pub use curve::P384Config;

#[cfg(feature = "k256")]
pub type K256 = Ecdsa<K256Config>;
#[cfg(feature = "p256")]
pub type P256 = Ecdsa<P256Config>;
#[cfg(feature = "p384")]
pub type P384 = Ecdsa<P384Config>;

#[cfg(feature = "k256")]
pub type DefaultCurve = K256Config;
#[cfg(all(not(feature = "k256"), feature = "p256"))]
pub type DefaultCurve = P256Config;
#[cfg(all(not(feature = "k256"), not(feature = "p256"), feature = "p384"))]
pub type DefaultCurve = P384Config;

/// Describes how to interact with a specific ECDSA curve implementation.
pub trait CurveConfig: Copy + Debug + PartialEq + Eq {
    type Signature: Clone + Debug + Eq + Send + Sync;
    type SigningKey: Clone + Debug + Send + Sync;
    type VerifyingKey: Clone + Debug + Eq + Send + Sync;

    const PRIVATE_KEY_TYPE: &'static str;
    const PUBLIC_KEY_TYPE: &'static str;

    #[cfg(feature = "rand")]
    fn signing_key_random<R>(rng: &mut R) -> Self::SigningKey
    where
        R: RngCore + CryptoRng;

    fn signing_key_from_bytes(bytes: &[u8]) -> Result<Self::SigningKey, SignatureError>;
    fn signing_key_to_bytes(key: &Self::SigningKey) -> Vec<u8>;

    fn verifying_key_from_signing(key: &Self::SigningKey) -> Self::VerifyingKey;
    fn verifying_key_from_sec1_bytes(bytes: &[u8]) -> Result<Self::VerifyingKey, SignatureError>;
    fn verifying_key_to_sec1_bytes(key: &Self::VerifyingKey) -> Vec<u8>;

    fn signature_from_bytes(bytes: &[u8]) -> Result<Self::Signature, SignatureError>;
    fn signature_to_bytes(signature: &Self::Signature) -> Vec<u8>;

    fn sign(key: &Self::SigningKey, msg: &[u8]) -> Result<Self::Signature, SignatureError>;
    fn verify(
        key: &Self::VerifyingKey,
        msg: &[u8],
        signature: &Self::Signature,
    ) -> Result<(), SignatureError>;
}

/// ECDSA signature wrapper parameterized by a curve configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature<C: CurveConfig>(C::Signature);

impl<C: CurveConfig> Signature<C> {
    pub fn new(inner: C::Signature) -> Self {
        Self(inner)
    }

    pub fn inner(&self) -> &C::Signature {
        &self.0
    }

    pub fn into_inner(self) -> C::Signature {
        self.0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        C::signature_to_bytes(&self.0)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, SignatureError> {
        C::signature_from_bytes(bytes).map(Self::new)
    }
}

impl<C: CurveConfig> PartialOrd for Signature<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<C: CurveConfig> Ord for Signature<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_vec().cmp(&other.to_vec())
    }
}

/// ECDSA signing key wrapper parameterized by a curve configuration.
#[derive(Clone, Debug)]
pub struct PrivateKey<C: CurveConfig>(C::SigningKey);

impl<C: CurveConfig> PrivateKey<C> {
    #[cfg(feature = "rand")]
    pub fn generate<R>(mut rng: R) -> Self
    where
        R: RngCore + CryptoRng,
    {
        Self(C::signing_key_random(&mut rng))
    }

    pub fn public_key(&self) -> PublicKey<C> {
        PublicKey::new(C::verifying_key_from_signing(&self.0))
    }

    pub fn sign(&self, msg: &[u8]) -> Signature<C> {
        self.try_sign(msg)
            .expect("deterministic ECDSA signing should not fail")
    }

    pub fn to_vec(&self) -> Vec<u8> {
        C::signing_key_to_bytes(&self.0)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, SignatureError> {
        C::signing_key_from_bytes(bytes).map(Self)
    }

    pub fn inner(&self) -> &C::SigningKey {
        &self.0
    }
}

impl<C: CurveConfig> Signer<Signature<C>> for PrivateKey<C> {
    fn try_sign(&self, msg: &[u8]) -> Result<Signature<C>, SignatureError> {
        C::sign(&self.0, msg).map(Signature::new)
    }
}

impl<C: CurveConfig> Keypair for PrivateKey<C> {
    type VerifyingKey = PublicKey<C>;

    fn verifying_key(&self) -> Self::VerifyingKey {
        self.public_key()
    }
}

/// ECDSA verifying key wrapper parameterized by a curve configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey<C: CurveConfig>(C::VerifyingKey);

impl<C: CurveConfig> PublicKey<C> {
    pub fn new(key: impl Into<C::VerifyingKey>) -> Self {
        Self(key.into())
    }

    pub fn to_vec(&self) -> Vec<u8> {
        C::verifying_key_to_sec1_bytes(&self.0)
    }

    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, SignatureError> {
        C::verifying_key_from_sec1_bytes(bytes).map(Self)
    }

    pub fn verify(&self, msg: &[u8], signature: &Signature<C>) -> Result<(), SignatureError> {
        C::verify(&self.0, msg, signature.inner())
    }

    pub fn inner(&self) -> &C::VerifyingKey {
        &self.0
    }
}

impl<C: CurveConfig> PartialOrd for PublicKey<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<C: CurveConfig> Ord for PublicKey<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_vec().cmp(&other.to_vec())
    }
}

impl<C: CurveConfig> Verifier<Signature<C>> for PublicKey<C> {
    fn verify(&self, msg: &[u8], signature: &Signature<C>) -> Result<(), SignatureError> {
        C::verify(&self.0, msg, signature.inner())
    }
}

/// Generic ECDSA signing scheme parameterized by the chosen curve configuration.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ecdsa<C: CurveConfig>(PhantomData<C>);

impl<C: CurveConfig> Default for Ecdsa<C> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<C: CurveConfig> Ecdsa<C> {
    #[cfg(feature = "rand")]
    pub fn generate_keypair<R>(rng: R) -> PrivateKey<C>
    where
        R: RngCore + CryptoRng,
    {
        PrivateKey::generate(rng)
    }
}

impl<C: CurveConfig> SigningScheme for Ecdsa<C> {
    type DecodingError = SignatureError;
    type Signature = Signature<C>;
    type PublicKey = PublicKey<C>;
    type PrivateKey = PrivateKey<C>;

    fn encode_signature(signature: &Self::Signature) -> Vec<u8> {
        signature.to_vec()
    }

    fn decode_signature(bytes: &[u8]) -> Result<Self::Signature, Self::DecodingError> {
        Signature::from_slice(bytes)
    }
}

#[cfg(all(test, feature = "serde", feature = "k256"))]
mod tests {
    use super::{PrivateKey, PublicKey, Signature};
    use crate::K256Config;

    use serde_json::{from_str, to_string};

    #[test]
    fn k256_serialization_roundtrip() {
        let private_key_bytes = [0x11u8; 32];
        let private_key = PrivateKey::<K256Config>::from_slice(&private_key_bytes)
            .expect("construct k256 private key");

        let serialized_private = to_string(&private_key).expect("serialize private key");
        let decoded_private: PrivateKey<K256Config> =
            from_str(&serialized_private).expect("deserialize private key");
        assert_eq!(private_key.to_vec(), decoded_private.to_vec());

        let public_key = decoded_private.public_key();
        let serialized_public = to_string(&public_key).expect("serialize public key");
        let decoded_public: PublicKey<K256Config> =
            from_str(&serialized_public).expect("deserialize public key");
        assert_eq!(public_key.to_vec(), decoded_public.to_vec());

        let message = b"malachite-k256-test";
        let signature = decoded_private.sign(message);
        let serialized_signature = to_string(&signature).expect("serialize signature");
        let decoded_signature: Signature<K256Config> =
            from_str(&serialized_signature).expect("deserialize signature");
        assert_eq!(signature.to_vec(), decoded_signature.to_vec());

        decoded_public
            .verify(message, &decoded_signature)
            .expect("signature verifies against round-tripped key");
    }
}
