//! Peer type classification for network peers

use std::fmt::Write;

use malachitebft_metrics::prometheus::encoding::EncodeLabelValue;

/// Type of peer for labeling and scoring
/// Note: This will change in the future when we can detect validator peers
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum PeerType {
    /// Persistent peer
    PersistentPeer,
    /// Full node
    FullNode,
}

impl EncodeLabelValue for PeerType {
    fn encode(
        &self,
        encoder: &mut malachitebft_metrics::prometheus::encoding::LabelValueEncoder,
    ) -> Result<(), std::fmt::Error> {
        match self {
            PeerType::PersistentPeer => encoder.write_str("persistent_peer"),
            PeerType::FullNode => encoder.write_str("full_node"),
        }
    }
}

impl From<bool> for PeerType {
    fn from(is_persistent: bool) -> Self {
        if is_persistent {
            PeerType::PersistentPeer
        } else {
            PeerType::FullNode
        }
    }
}

impl PeerType {
    pub fn is_persistent(&self) -> bool {
        matches!(self, PeerType::PersistentPeer)
    }
}
