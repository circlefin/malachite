//! Types for the Validator Proof protocol.

use serde::{Deserialize, Serialize};

/// Internal verification result.
///
/// This is used internally for tracking - the wire protocol is one-way
/// and doesn't send any response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum VerificationResult {
    /// Proof received and verified successfully.
    Valid = 0,
    /// Proof validation failed (decode, peer_id mismatch, or invalid signature).
    Invalid = 1,
}

impl VerificationResult {
    /// Whether the verification succeeded.
    pub fn is_verified(self) -> bool {
        matches!(self, Self::Valid)
    }
}
