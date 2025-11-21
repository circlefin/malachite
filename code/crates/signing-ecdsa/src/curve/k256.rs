use alloc::vec::Vec;

use crate::CurveConfig;
use crate::SignatureError;
use signature::{Signer, Verifier};

#[cfg(feature = "rand")]
use rand::{CryptoRng, RngCore};

use k256::ecdsa::{
    Signature as K256Signature, SigningKey as K256SigningKey, VerifyingKey as K256VerifyingKey,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct K256Config;

impl CurveConfig for K256Config {
    type Signature = K256Signature;
    type SigningKey = K256SigningKey;
    type VerifyingKey = K256VerifyingKey;

    const PRIVATE_KEY_TYPE: &'static str = "tendermint/PrivKeySecp256k1";
    const PUBLIC_KEY_TYPE: &'static str = "tendermint/PubKeySecp256k1";

    #[cfg(feature = "rand")]
    fn signing_key_random<R>(rng: &mut R) -> Self::SigningKey
    where
        R: RngCore + CryptoRng,
    {
        Self::SigningKey::random(rng)
    }

    fn signing_key_from_bytes(bytes: &[u8]) -> Result<Self::SigningKey, SignatureError> {
        Self::SigningKey::from_slice(bytes).map_err(|_| SignatureError::new())
    }

    fn signing_key_to_bytes(key: &Self::SigningKey) -> Vec<u8> {
        key.to_bytes().to_vec()
    }

    fn verifying_key_from_signing(key: &Self::SigningKey) -> Self::VerifyingKey {
        Self::VerifyingKey::from(key)
    }

    fn verifying_key_from_sec1_bytes(bytes: &[u8]) -> Result<Self::VerifyingKey, SignatureError> {
        Self::VerifyingKey::from_sec1_bytes(bytes).map_err(|_| SignatureError::new())
    }

    fn verifying_key_to_sec1_bytes(key: &Self::VerifyingKey) -> Vec<u8> {
        key.to_encoded_point(true).as_bytes().to_vec()
    }

    fn signature_from_bytes(bytes: &[u8]) -> Result<Self::Signature, SignatureError> {
        Self::Signature::from_slice(bytes).map_err(|_| SignatureError::new())
    }

    fn signature_to_bytes(signature: &Self::Signature) -> Vec<u8> {
        signature.to_bytes().to_vec()
    }

    fn sign(key: &Self::SigningKey, msg: &[u8]) -> Result<Self::Signature, SignatureError> {
        key.try_sign(msg).map_err(|_| SignatureError::new())
    }

    fn verify(
        key: &Self::VerifyingKey,
        msg: &[u8],
        signature: &Self::Signature,
    ) -> Result<(), SignatureError> {
        key.verify(msg, signature)
            .map_err(|_| SignatureError::new())
    }
}
