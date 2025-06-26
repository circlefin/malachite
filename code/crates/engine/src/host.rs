use bytes::Bytes;
use std::time::Duration;

use derive_where::derive_where;
use ractor::{ActorRef, RpcReplyPort};

use malachitebft_core_consensus::{Role, VoteExtensionError};
use malachitebft_core_types::{CommitCertificate, Context, Round, ValueId, VoteExtensions};
use malachitebft_sync::{PeerId, RawDecidedValue};

use crate::util::streaming::StreamMessage;

pub use malachitebft_core_consensus::{LocallyProposedValue, ProposedValue};

/// A reference to the host actor.
pub type HostRef<Ctx> = ActorRef<HostMsg<Ctx>>;

/// What to do next after a decision.
#[derive_where(Debug)]
pub enum Next<Ctx: Context> {
    /// Start at the given height with the given validator set.
    Start(Ctx::Height, Ctx::ValidatorSet),

    Restart(
        /// Restart at the given height with the given validator set.
        Ctx::Height,
        Ctx::ValidatorSet,
    ),
}

/// Messages that need to be handled by the host actor.
#[derive_where(Debug)]
pub enum HostMsg<Ctx: Context> {
    /// Consensus is ready
    ConsensusReady {
        /// Use this reply port to instruct consensus to start the first height.
        reply_to: RpcReplyPort<(Ctx::Height, Ctx::ValidatorSet)>,
    },

    /// Consensus has started a new round.
    StartedRound {
        /// The height at which the round started.
        height: Ctx::Height,
        /// The round number that started.
        round: Round,
        /// The address of the proposer for this round.
        proposer: Ctx::Address,
        /// The role of the node in this round.
        role: Role,
        /// Use this reply port to send the undecided values that were already seen for this
        /// round. This is needed when recovering from a crash.
        ///
        /// The application MUST reply immediately with the values it has, or with an empty vector.
        reply_to: RpcReplyPort<Vec<ProposedValue<Ctx>>>,
    },

    /// Request to build a local value to propose
    GetValue {
        /// The height at which the value should be proposed.
        height: Ctx::Height,
        /// The round in which the value should be proposed.
        round: Round,
        /// The amount of time the application has to build the value.
        timeout: Duration,
        /// Use this reply port to send the value that was built.
        reply_to: RpcReplyPort<LocallyProposedValue<Ctx>>,
    },

    /// ExtendVote allows the application to extend the pre-commit vote with arbitrary data.
    ///
    /// When consensus is preparing to send a pre-commit vote, it first calls `ExtendVote`.
    /// The application then returns a blob of data called a vote extension.
    /// This data is opaque to the consensus algorithm but can contain application-specific information.
    /// The proposer of the next block will receive all vote extensions along with the commit certificate.
    ExtendVote {
        /// The height at which the vote is being extended.
        height: Ctx::Height,
        /// The round in which the vote is being extended.
        round: Round,
        /// The ID of the value that is being voted on.
        value_id: ValueId<Ctx>,
        /// The vote extension to be added to the vote, if any.
        reply_to: RpcReplyPort<Option<Ctx::Extension>>,
    },

    /// Verify a vote extension
    ///
    /// If the vote extension is deemed invalid, the vote it was part of
    /// will be discarded altogether.
    VerifyVoteExtension {
        /// The height for which the vote is.
        height: Ctx::Height,
        /// The round for which the vote is.
        round: Round,
        /// The ID of the value that the vote extension is for.
        value_id: ValueId<Ctx>,
        /// The vote extension to verify.
        extension: Ctx::Extension,
        /// Use this reply port to send the result of the verification.
        reply_to: RpcReplyPort<Result<(), VoteExtensionError>>,
    },

    /// Request to restream an existing block/value from Driver
    RestreamValue {
        /// The height at which the value was proposed.
        height: Ctx::Height,
        /// The round in which the value was proposed.
        round: Round,
        /// The round in which the value was valid.
        valid_round: Round,
        /// The address of the proposer of the value.
        address: Ctx::Address,
        /// The ID of the value to restream.
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

        /// Use this reply port to instruct consensus to start the next height.
        reply_to: RpcReplyPort<Next<Ctx>>,
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
