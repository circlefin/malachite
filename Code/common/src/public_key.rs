use core::fmt::Debug;

use signature::{Signer, Verifier};

/// Defines the requirements for a private key type.
pub trait PrivateKey
where
    Self: Clone + Debug + Signer<Self::Signature>,
{
    type Signature: Clone + Debug + PartialEq + Eq;
    type PublicKey: PublicKey<Signature = Self::Signature>;

    fn public_key(&self) -> Self::PublicKey;
}

/// Defines the requirements for a public key type.
pub trait PublicKey
where
    Self: Clone + Debug + PartialEq + Eq + Verifier<Self::Signature>,
{
    type Signature: Clone + Debug + PartialEq + Eq;
}
