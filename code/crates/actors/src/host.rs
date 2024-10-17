use std::time::Duration;

use derive_where::derive_where;
use libp2p::PeerId;
use ractor::{ActorRef, RpcReplyPort};

use malachite_common::{Context, Extension, Round, SignedVote};

/// This is the value that the application constructed
/// and has finished streaming on gossip.
///
/// This is passed back to the consensus layer.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct LocallyProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Ctx::Value,
    pub extension: Option<Extension>,
}

impl<Ctx: Context> LocallyProposedValue<Ctx> {
    pub fn new(
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        extension: Option<Extension>,
    ) -> Self {
        Self {
            height,
            round,
            value,
            extension,
        }
    }
}

/// A value to propose that has just been received.
pub use malachite_consensus::ProposedValue;

use crate::consensus::ConsensusRef;
use crate::util::streaming::StreamMessage;

/// A reference to the host actor.
pub type HostRef<Ctx> = ActorRef<HostMsg<Ctx>>;

/// Messages that need to be handled by the host actor.
pub enum HostMsg<Ctx: Context> {
    /// Consensus has started a new round.
    StartRound {
        height: Ctx::Height,
        round: Round,
        proposer: Ctx::Address,
    },

    /// Request to build a local block/value from Driver
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        address: Ctx::Address,
        reply_to: RpcReplyPort<LocallyProposedValue<Ctx>>,
    },

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
    Decide {
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        commits: Vec<SignedVote<Ctx>>,
        consensus: ConsensusRef<Ctx>,
    },
}
