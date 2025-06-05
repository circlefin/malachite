use bytes::Bytes;
use std::time::Duration;

use derive_where::derive_where;
use ractor::{ActorRef, RpcReplyPort};

use malachitebft_core_consensus::{Role, VoteExtensionError};
use malachitebft_core_types::{CommitCertificate, Context, Round, ValueId, VoteExtensions};
use malachitebft_sync::{PeerId, RawDecidedValue};

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
        role: Role,
    },

    /// Request to build a local value to propose
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout: Duration,
        reply_to: RpcReplyPort<LocallyProposedValue<Ctx>>,
    },

    /// ExtendVote allows the application to extend the pre-commit vote with arbitrary data.
    ///
    /// When consensus is preparing to send a pre-commit vote, it first calls `ExtendVote`.
    /// The application then returns a blob of data called a vote extension.
    /// This data is opaque to the consensus algorithm but can contain application-specific information.
    /// The proposer of the next block will receive all vote extensions along with the commit certificate.
    ExtendVote {
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        reply_to: RpcReplyPort<Option<Ctx::Extension>>,
    },

    /// Verify a vote extension
    ///
    /// If the vote extension is deemed invalid, the vote it was part of
    /// will be discarded altogether.
    VerifyVoteExtension {
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        extension: Ctx::Extension,
        reply_to: RpcReplyPort<Result<(), VoteExtensionError>>,
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
        reply_to: RpcReplyPort<Option<Ctx::ValidatorSet>>,
    },

    /// Consensus has decided on a value.
    Decided {
        /// The commit certificate containing the ID of the value that was decided on,
        /// the the height and round at which it was decided, and the aggregated signatures
        /// of the validators that committed to it.
        certificate: CommitCertificate<Ctx>,

        /// Vote extensions that were received for this height.
        extensions: VoteExtensions<Ctx>,

        /// Reference to the `Consensus` actor for starting a new height.
        consensus: ConsensusRef<Ctx>,
    },

    // Retrieve decided value from the block store
    GetDecidedValue {
        height: Ctx::Height,
        reply_to: RpcReplyPort<Option<RawDecidedValue<Ctx>>>,
    },

    // Process a value synced from another node via the ValueSync protocol.
    //
    // If the encoded value within is valid, the host MUST reply with that value to be proposed.
    ProcessSyncedValue {
        height: Ctx::Height,
        round: Round,
        validator_address: Ctx::Address,
        value_bytes: Bytes,
        reply_to: RpcReplyPort<ProposedValue<Ctx>>,
    },
}
