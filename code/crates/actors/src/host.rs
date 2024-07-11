use std::time::Duration;

use derive_where::derive_where;
use ractor::{ActorRef, RpcReplyPort};

use malachite_common::{Context, Round, SignedVote};

use crate::consensus::ConsensusRef;

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct LocallyProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Ctx::Value,
}

impl<Ctx: Context> LocallyProposedValue<Ctx> {
    pub fn new(height: Ctx::Height, round: Round, value: Ctx::Value) -> Self {
        Self {
            height,
            round,
            value,
        }
    }
}

/// A value to propose that has just been received.
pub type ReceivedProposedValue<Ctx> = malachite_consensus::Block<Ctx>;

/// A reference to the host actor.
pub type HostRef<Ctx> = ActorRef<HostMsg<Ctx>>;

/// Messages that need to be handled by the host actor.
pub enum HostMsg<Ctx: Context> {
    /// Request to build a local block/value from Driver
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        consensus: ConsensusRef<Ctx>,
        address: Ctx::Address,
        reply_to: RpcReplyPort<LocallyProposedValue<Ctx>>,
    },

    /// BlockPart received <-- consensus <-- gossip
    ReceivedBlockPart {
        block_part: Ctx::BlockPart,
        reply_to: RpcReplyPort<ReceivedProposedValue<Ctx>>,
    },

    /// Retrieve a block/value for which all parts have been received
    GetReceivedValue {
        height: Ctx::Height,
        round: Round,
        reply_to: RpcReplyPort<Option<ReceivedProposedValue<Ctx>>>,
    },

    /// Get the validator set at a given height
    GetValidatorSet {
        height: Ctx::Height,
        reply_to: RpcReplyPort<Ctx::ValidatorSet>,
    },

    // Decided value
    DecidedOnValue {
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        commits: Vec<SignedVote<Ctx>>,
    },
}
