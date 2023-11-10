use malachite_common::SigningScheme;
use rand::{CryptoRng, RngCore};
use secrecy::{CloneableSecret, DebugSecret, Zeroize};
use signature::{Keypair, Signer, Verifier};

pub use ed25519_consensus::Signature;

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
    type Signature = Signature;
    type PublicKey = PublicKey;
    type PrivateKey = PrivateKey;
}

#[derive(Clone, Debug)]
pub struct PrivateKey(ed25519_consensus::SigningKey);

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

impl Zeroize for PrivateKey {
    fn zeroize(&mut self) {
        self.0.zeroize()
    }
}

impl DebugSecret for PrivateKey {}
impl CloneableSecret for PrivateKey {}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PublicKey(ed25519_consensus::VerificationKey);

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
}

impl Verifier<Signature> for PublicKey {
    fn verify(&self, msg: &[u8], signature: &Signature) -> Result<(), signature::Error> {
        self.0
            .verify(signature, msg)
            .map_err(|_| signature::Error::new())
    }
}
