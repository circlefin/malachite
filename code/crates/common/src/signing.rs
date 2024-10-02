use alloc::vec::Vec;
use core::fmt::{Debug, Display};

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
    /// Errors that can occur when decoding a signature from a byte array.
    type DecodingError: Display;

    /// The type of signatures produced by this signing scheme.
    type Signature: Clone + Debug + Eq + Ord + Send + Sync;

    /// The type of public keys produced by this signing scheme.
    type PublicKey: Clone + Debug + Eq + Send + Sync;

    /// The type of private keys produced by this signing scheme.
    type PrivateKey: Clone + Send + Sync;

    /// Decode a signature from a byte array.
    fn decode_signature(bytes: &[u8]) -> Result<Self::Signature, Self::DecodingError>;

    /// Encode a signature to a byte array.
    fn encode_signature(signature: &Self::Signature) -> Vec<u8>;
}
