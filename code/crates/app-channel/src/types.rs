//! Re-export of all types required to build a Malachite application.

pub use malachite_actors::host::LocallyProposedValue;
pub use malachite_consensus::{ConsensusMsg, ProposedValue, SignedConsensusMsg};
pub use malachite_node::Node;
pub use malachite_peer::PeerId;

pub mod core {
    pub use malachite_common::*;
}

pub mod config {
    pub use malachite_config::*;
}

pub mod metrics {
    pub use malachite_metrics::*;
}

pub use libp2p_identity::Keypair;

pub mod streaming {
    pub use malachite_actors::util::streaming::StreamMessage;
}

pub mod sync {
    pub use malachite_blocksync::{Request, Response, Status, SyncedBlock};
}

pub mod codec {
    pub use malachite_actors::block_sync::BlockSyncCodec;
    pub use malachite_actors::consensus::ConsensusCodec;
    pub use malachite_actors::wal::WalCodec;
    pub use malachite_codec::Codec;
}
