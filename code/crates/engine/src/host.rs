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

    /// Restart at the given height with the given validator set.
    Restart(Ctx::Height, Ctx::ValidatorSet),
}

/// Messages that need to be handled by the host actor.
#[derive_where(Debug)]
pub enum HostMsg<Ctx: Context> {
    /// Notifies the application that consensus is ready.
    ///
    /// The application MUST reply with a message to instruct
    /// consensus to start at a given height.
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
    ///
    /// The application MUST reply to this message with the requested value
    /// within the specified timeout duration.
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

    /// Requests the application to re-stream a proposal that it has already seen.
    ///
    /// The application MUST re-publish again all the proposal parts pertaining
    /// to that value by sending [`NetworkMsg::PublishProposalPart`] messages through
    /// the [`Channels::network`] channel.
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

    /// Requests the earliest height available in the history maintained by the application.
    ///
    /// The application MUST respond with its earliest available height.
    GetHistoryMinHeight { reply_to: RpcReplyPort<Ctx::Height> },

    /// Notifies the application that consensus has received a proposal part over the network.
    ///
    /// If this part completes the full proposal, the application MUST respond
    /// with the complete proposed value. Otherwise, it MUST respond with `None`.
    ReceivedProposalPart {
        from: PeerId,
        part: StreamMessage<Ctx::ProposalPart>,
        reply_to: RpcReplyPort<ProposedValue<Ctx>>,
    },

    /// Notifies the application that consensus has decided on a value.
    ///
    /// This message includes a commit certificate containing the ID of
    /// the value that was decided on, the height and round at which it was decided,
    /// and the aggregated signatures of the validators that committed to it.
    /// It also includes to the vote extensions received for that height.
    ///
    /// In response to this message, the application MUST send a [`Next`]
    /// message back to consensus, instructing it to either start the next height if
    /// the application was able to commit the decided value, or to restart the current height
    /// otherwise.
    ///
    /// If the application does not reply, consensus will stall.
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

    /// Requests a previously decided value from the application's storage.
    ///
    /// The application MUST respond with that value if available, or `None` otherwise.
    GetDecidedValue {
        /// Height of the decided value to retrieve
        height: Ctx::Height,
        /// Channel for sending back the decided value
        reply_to: RpcReplyPort<Option<RawDecidedValue<Ctx>>>,
    },

    /// Notifies the application that a value has been synced from the network.
    /// This may happen when the node is catching up with the network.
    ///
    /// If a value can be decoded from the bytes provided, then the application MUST reply
    /// to this message with the decoded value. Otherwise, it MUST reply with `None`.
    ProcessSyncedValue {
        /// Height of the synced value
        height: Ctx::Height,
        /// Round of the synced value
        round: Round,
        /// Address of the original proposer
        proposer: Ctx::Address,
        /// Raw encoded value data
        value_bytes: Bytes,
        /// Channel for sending back the proposed value, if successfully decoded
        /// or `None` if the value could not be decoded
        reply_to: RpcReplyPort<ProposedValue<Ctx>>,
    },
}
