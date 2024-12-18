use bytes::Bytes;
use std::time::Duration;

use derive_where::derive_where;
use ractor::{ActorRef, RpcReplyPort};

use malachite_consensus::PeerId;
use malachite_core_types::{CommitCertificate, Context, Round, SignedExtension, ValueId};
use malachite_sync::DecidedValue;

use crate::consensus::ConsensusRef;
use crate::util::streaming::StreamMessage;

/// A value to propose that has just been received.
pub use malachite_consensus::ProposedValue;

/// This is the value that the application constructed
/// and has finished streaming on gossip.
///
/// This is passed back to the consensus layer.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct LocallyProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Ctx::Value,
    pub extension: Option<SignedExtension<Ctx>>,
}

impl<Ctx: Context> LocallyProposedValue<Ctx> {
    pub fn new(
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        extension: Option<SignedExtension<Ctx>>,
    ) -> Self {
        Self {
            height,
            round,
            value,
            extension,
        }
    }
}

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

    // Retrieve decided block from the block store
    GetDecidedValue {
        height: Ctx::Height,
        reply_to: RpcReplyPort<Option<DecidedValue<Ctx>>>,
    },

    // Synced block
    ProcessSyncedValue {
        height: Ctx::Height,
        round: Round,
        validator_address: Ctx::Address,
        value_bytes: Bytes,
        reply_to: RpcReplyPort<ProposedValue<Ctx>>,
    },
}
