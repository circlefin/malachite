//! Validator Proof type for the Proof-of-Validator protocol.

use alloc::vec::Vec;
use derive_where::derive_where;

use crate::{Context, Signature};

/// Separator bytes for Proof-of-Validator signatures.
/// The 3-byte ASCII string "PoV" (0x50 0x6F 0x56).
const POV_SEPARATOR: &[u8] = b"PoV";

/// A proof that a libp2p peer ID is controlled by a validator.
///
/// This allows nodes to cryptographically verify that a peer claiming to be
/// a validator actually controls the corresponding consensus private key.
///
/// The proof binds a network ID (peer_id) to a consensus public key,
/// signed by the corresponding consensus private key. This allows immediate
/// signature verification without needing to look up the public key from the
/// validator set.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ValidatorProof<Ctx: Context> {
    /// The validator's consensus public key (raw bytes)
    pub public_key: Vec<u8>,
    /// The libp2p peer ID bytes (network_id)
    pub peer_id: Vec<u8>,
    /// Signature over (public_key, peer_id) using the validator's consensus key
    pub signature: Signature<Ctx>,
}

impl<Ctx: Context> ValidatorProof<Ctx> {
    /// Creates a new `ValidatorProof`.
    pub fn new(public_key: Vec<u8>, peer_id: Vec<u8>, signature: Signature<Ctx>) -> Self {
        Self {
            public_key,
            peer_id,
            signature,
        }
    }

    /// Returns the bytes to be signed for this proof.
    ///
    /// Format: SEPARATOR || len(public_key) || public_key || len(peer_id) || peer_id
    ///
    /// Where:
    /// - SEPARATOR is "PoV" (0x50 0x6F 0x56)
    /// - len() is encoded as 4 bytes (u32 big-endian)
    pub fn signing_bytes(public_key: &[u8], peer_id: &[u8]) -> Vec<u8> {
        let mut bytes =
            Vec::with_capacity(POV_SEPARATOR.len() + 4 + public_key.len() + 4 + peer_id.len());
        bytes.extend_from_slice(POV_SEPARATOR);
        bytes.extend_from_slice(&(public_key.len() as u32).to_be_bytes());
        bytes.extend_from_slice(public_key);
        bytes.extend_from_slice(&(peer_id.len() as u32).to_be_bytes());
        bytes.extend_from_slice(peer_id);
        bytes
    }
}
