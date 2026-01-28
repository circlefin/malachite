//! Peer type classification for network peers

use std::fmt::Write;
use std::hash::{Hash, Hasher};

use malachitebft_metrics::prometheus::encoding::EncodeLabelValue;

/// Type of peer for labeling and scoring
/// Peers can be validators (in current validator set), persistent (configured), both, or neither (full nodes)
///
/// Note: Hash and PartialEq are implemented manually to be consistent with EncodeLabelValue.
/// Two PeerTypes that produce the same Prometheus label must have the same Hash and be equal.
#[derive(Clone, Copy, Debug)]
pub struct PeerType {
    is_persistent: bool,
    is_validator: bool,
}

impl PartialEq for PeerType {
    fn eq(&self, other: &Self) -> bool {
        // Two PeerTypes are equal if they produce the same primary_type_str
        self.primary_type_str() == other.primary_type_str()
    }
}

impl Eq for PeerType {}

impl Hash for PeerType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash based on primary_type_str to be consistent with PartialEq and EncodeLabelValue
        self.primary_type_str().hash(state);
    }
}

impl PeerType {
    /// Create a new PeerType with specified persistent and validator status
    pub fn new(is_persistent: bool, is_validator: bool) -> Self {
        Self {
            is_persistent,
            is_validator,
        }
    }

    /// Create a new PeerType with updated validator status, preserving persistent status
    pub fn with_validator_status(self, is_validator: bool) -> Self {
        Self {
            is_persistent: self.is_persistent,
            is_validator,
        }
    }

    /// Get the primary type for display/metrics (prioritize validator > persistent > full node)
    pub fn primary_type_str(&self) -> &'static str {
        match (self.is_validator, self.is_persistent) {
            (true, _) => "validator",           // Validator (may also be persistent)
            (false, true) => "persistent_peer", // Persistent but not validator
            (false, false) => "full_node",      // Neither
        }
    }

    pub fn is_persistent(&self) -> bool {
        self.is_persistent
    }

    pub fn is_validator(&self) -> bool {
        self.is_validator
    }
}

impl EncodeLabelValue for PeerType {
    fn encode(
        &self,
        encoder: &mut malachitebft_metrics::prometheus::encoding::LabelValueEncoder,
    ) -> Result<(), std::fmt::Error> {
        encoder.write_str(self.primary_type_str())
    }
}
