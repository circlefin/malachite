use bytes::Bytes;
use std::time::Duration;

use derive_where::derive_where;
use ractor::{ActorRef, RpcReplyPort};

use malachitebft_core_consensus::PeerId;
use malachitebft_core_types::{CommitCertificate, Context, Round, ValueId};
use malachitebft_sync::RawDecidedValue;

use crate::consensus::ConsensusRef;
use crate::util::streaming::StreamMessage;

pub use malachitebft_core_consensus::{LocallyProposedValue, ProposedValue};

/// A reference to the host actor.
pub type HostRef<Ctx> = ActorRef<HostMsg<Ctx>>;

/// Messages that need to be handled by the host actor.
#[derive_where(Debug)]
pub enum HostMsg<Ctx: Context> {
    /// Consensus is ready
    ConsensusReady(ConsensusRef<Ctx>),

    /// Consensus has started a new round.
    StartedRound {
        height: Ctx::Height,
        round: Round,
        proposer: Ctx::Address,
    },

    /// Request to build a local block/value from Driver
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout: Duration,
        reply_to: RpcReplyPort<LocallyProposedValue<Ctx>>,
    },

    /// Request to restream an existing block/value from Driver
    RestreamValue {
        height: Ctx::Height,
        round: Round,
        valid_round: Round,
        address: Ctx::Address,
        value_id: ValueId<Ctx>,
    },

    /// Request the earliest block height in the block store
    GetHistoryMinHeight { reply_to: RpcReplyPort<Ctx::Height> },

    /// ProposalPart received <-- consensus <-- gossip
    ReceivedProposalPart {
        from: PeerId,
        part: StreamMessage<Ctx::ProposalPart>,
        reply_to: RpcReplyPort<ProposedValue<Ctx>>,
    },

    /// Get the validator set at a given height
    GetValidatorSet {
        height: Ctx::Height,
        reply_to: RpcReplyPort<Ctx::ValidatorSet>,
    },

    // Consensus has decided on a value
    Decided {
        certificate: CommitCertificate<Ctx>,
        consensus: ConsensusRef<Ctx>,
    },

    // Retrieve decided value from the block store
    GetDecidedValue {
        height: Ctx::Height,
        reply_to: RpcReplyPort<Option<RawDecidedValue<Ctx>>>,
    },

    // Process a value synced from another node via the ValueSync protocol.
    // If the encoded value within is valid, reply with that value to be proposed.
    ProcessSyncedValue {
        height: Ctx::Height,
        round: Round,
        validator_address: Ctx::Address,
        value_bytes: Bytes,
        reply_to: RpcReplyPort<ProposedValue<Ctx>>,
    },

    /// A peer joined our local view of the network.
    /// In a gossip network, there is no guarantee that we will ever see all peers,
    /// as we are typically only connected to a subset of the network (i.e. in our mesh).
    PeerJoined {
        /// The ID of the peer that joined
        peer_id: PeerId,
    },

    /// A peer left our local view of the network.
    /// In a gossip network, there is no guarantee that this means that this peer
    /// has left the whole network altogether, just that it is not part of the subset
    /// of the network that we are connected to (i.e. our mesh).
    PeerLeft {
        /// The ID of the peer that left
        peer_id: PeerId,
    },
}
