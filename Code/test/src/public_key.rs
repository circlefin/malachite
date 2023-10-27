use ed25519_consensus::{Signature, SigningKey, VerificationKey};

use malachite_common::{PrivateKey, PublicKey};
use rand::{CryptoRng, RngCore};
use signature::{Signer, Verifier};

pub type Ed25519Signature = Signature;

#[derive(Clone, Debug)]
pub struct Ed25519PrivateKey(SigningKey);

impl Ed25519PrivateKey {
    pub fn generate<R>(rng: R) -> Self
    where
        R: RngCore + CryptoRng,
    {
        let signing_key = SigningKey::new(rng);

        Self(signing_key)
    }

    pub fn public_key(&self) -> Ed25519PublicKey {
        Ed25519PublicKey::new(self.0.verification_key())
    }
}

impl PrivateKey for Ed25519PrivateKey {
    type Signature = Signature;
    type PublicKey = Ed25519PublicKey;

    fn public_key(&self) -> Self::PublicKey {
        self.public_key()
    }
}

impl Signer<Signature> for Ed25519PrivateKey {
    fn try_sign(&self, msg: &[u8]) -> Result<Signature, signature::Error> {
        Ok(self.0.sign(msg))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ed25519PublicKey(VerificationKey);

impl Ed25519PublicKey {
    pub fn new(key: impl Into<VerificationKey>) -> Self {
        Self(key.into())
    }

    pub fn hash(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.0.as_bytes());
        hasher.finalize().into()
    }
}

impl PublicKey for Ed25519PublicKey {
    type Signature = Signature;
}

impl Verifier<Signature> for Ed25519PublicKey {
    fn verify(&self, msg: &[u8], signature: &Signature) -> Result<(), signature::Error> {
        self.0
            .verify(signature, msg)
            .map_err(|_| signature::Error::new())
    }
}
