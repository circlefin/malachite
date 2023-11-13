use core::fmt::Debug;

use signature::{Keypair, Signer, Verifier};

pub trait SigningScheme
where
    Self: Clone + Debug + Eq,
{
    type Signature: Clone + Debug + Eq;

    type PublicKey: Clone + Debug + Eq + Verifier<Self::Signature>;

    type PrivateKey: Clone + Signer<Self::Signature> + Keypair<VerifyingKey = Self::PublicKey>;
}
