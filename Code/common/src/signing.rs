use core::fmt::Debug;

use signature::{Keypair, Signer, Verifier};

/// A signing scheme that can be used to sign votes and verify such signatures.
///
/// This trait is used to abstract over the signature scheme used by the consensus engine.
///
/// An example of a signing scheme is the Ed25519 signature scheme,
/// eg. as implemented in the [`ed25519-consensus`][ed25519-consensus] crate.
///
/// [ed25519-consensus]: https://crates.io/crates/ed25519-consensus
pub trait SigningScheme
where
    Self: Clone + Debug + Eq,
{
    /// The type of signatures produced by this signing scheme.
    type Signature: Clone + Debug + Eq;

    /// The type of public keys produced by this signing scheme.
    type PublicKey: Clone + Debug + Eq + Verifier<Self::Signature>;

    /// The type of private keys produced by this signing scheme.
    type PrivateKey: Clone + Signer<Self::Signature> + Keypair<VerifyingKey = Self::PublicKey>;
}
