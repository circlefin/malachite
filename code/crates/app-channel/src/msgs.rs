use std::time::Duration;

use bytes::Bytes;
use derive_where::derive_where;
use malachitebft_app::consensus::Role;
use malachitebft_app::types::core::ValueOrigin;
use malachitebft_engine::host::Next;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use malachitebft_app::consensus::VoteExtensionError;
use malachitebft_engine::consensus::Msg as ConsensusActorMsg;
use malachitebft_engine::network::Msg as NetworkActorMsg;
use malachitebft_engine::util::events::TxEvent;

use crate::app::types::core::{CommitCertificate, Context, Round, ValueId, VoteExtensions};
use crate::app::types::streaming::StreamMessage;
use crate::app::types::sync::RawDecidedValue;
use crate::app::types::{LocallyProposedValue, PeerId, ProposedValue};

pub type Reply<T> = oneshot::Sender<T>;

/// Channels created for application consumption
pub struct Channels<Ctx: Context> {
    /// Channel for receiving messages from consensus
    pub consensus: mpsc::Receiver<AppMsg<Ctx>>,
    /// Channel for sending messages to the networking layer
    pub network: mpsc::Sender<NetworkMsg<Ctx>>,
    /// Receiver of events, call `subscribe` to receive them
    pub events: TxEvent<Ctx>,
}

/// Messages sent from consensus to the application.
#[derive_where(Debug)]
pub enum AppMsg<Ctx: Context> {
    /// Notifies the application that consensus is ready.
    ///
    /// The application MUST reply with a message to instruct
    /// consensus to start at a given height.
    ConsensusReady(ConsensusReady<Ctx>),

    /// Notifies the application that a new consensus round has begun.
    StartedRound(StartedRound<Ctx>),

    /// Requests the application to build a value for consensus to propose.
    ///
    /// The application MUST reply to this message with the requested value
    /// within the specified timeout duration.
    GetValue(GetValue<Ctx>),

    /// ExtendVote allows the application to extend the pre-commit vote with arbitrary data.
    ///
    /// When consensus is preparing to send a pre-commit vote, it first calls `ExtendVote`.
    /// The application then returns a blob of data called a vote extension.
    /// This data is opaque to the consensus algorithm but can contain application-specific information.
    /// The proposer of the next block will receive all vote extensions along with the commit certificate.
    ExtendVote(ExtendVote<Ctx>),

    /// Verify a vote extension
    ///
    /// If the vote extension is deemed invalid, the vote it was part of
    /// will be discarded altogether.
    VerifyVoteExtension(VerifyVoteExtension<Ctx>),

    /// Requests the application to re-stream a proposal that it has already seen.
    ///
    /// The application MUST re-publish again all the proposal parts pertaining
    /// to that value by sending [`NetworkMsg::PublishProposalPart`] messages through
    /// the [`Channels::network`] channel.
    RestreamProposal(RestreamProposal<Ctx>),

    /// Requests the earliest height available in the history maintained by the application.
    ///
    /// The application MUST respond with its earliest available height.
    GetHistoryMinHeight(GetHistoryMinHeight<Ctx>),

    /// Notifies the application that consensus has received a proposal part over the network.
    ///
    /// If this part completes the full proposal, the application MUST respond
    /// with the complete proposed value. Otherwise, it MUST respond with `None`.
    ReceivedProposalPart(ReceivedProposalPart<Ctx>),

    /// Requests the validator set for a specific height
    GetValidatorSet(GetValidatorSet<Ctx>),

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
    Decided(Decided<Ctx>),

    /// Requests a previously decided value from the application's storage.
    ///
    /// The application MUST respond with that value if available, or `None` otherwise.
    GetDecidedValue(GetDecidedValue<Ctx>),

    /// Notifies the application that a value has been synced from the network.
    /// This may happen when the node is catching up with the network.
    ///
    /// If a value can be decoded from the bytes provided, then the application MUST reply
    /// to this message with the decoded value. Otherwise, it MUST reply with `None`.
    ProcessSyncedValue(ProcessSyncedValue<Ctx>),
}

impl<Ctx: Context> From<ConsensusReady<Ctx>> for AppMsg<Ctx> {
    fn from(value: ConsensusReady<Ctx>) -> Self {
        Self::ConsensusReady(value)
    }
}

impl<Ctx: Context> From<StartedRound<Ctx>> for AppMsg<Ctx> {
    fn from(value: StartedRound<Ctx>) -> Self {
        Self::StartedRound(value)
    }
}

impl<Ctx: Context> From<GetValue<Ctx>> for AppMsg<Ctx> {
    fn from(value: GetValue<Ctx>) -> Self {
        Self::GetValue(value)
    }
}

impl<Ctx: Context> From<ExtendVote<Ctx>> for AppMsg<Ctx> {
    fn from(value: ExtendVote<Ctx>) -> Self {
        Self::ExtendVote(value)
    }
}

impl<Ctx: Context> From<VerifyVoteExtension<Ctx>> for AppMsg<Ctx> {
    fn from(value: VerifyVoteExtension<Ctx>) -> Self {
        Self::VerifyVoteExtension(value)
    }
}

impl<Ctx: Context> From<RestreamProposal<Ctx>> for AppMsg<Ctx> {
    fn from(value: RestreamProposal<Ctx>) -> Self {
        Self::RestreamProposal(value)
    }
}

impl<Ctx: Context> From<GetHistoryMinHeight<Ctx>> for AppMsg<Ctx> {
    fn from(value: GetHistoryMinHeight<Ctx>) -> Self {
        Self::GetHistoryMinHeight(value)
    }
}

impl<Ctx: Context> From<ReceivedProposalPart<Ctx>> for AppMsg<Ctx> {
    fn from(value: ReceivedProposalPart<Ctx>) -> Self {
        Self::ReceivedProposalPart(value)
    }
}

impl<Ctx: Context> From<GetValidatorSet<Ctx>> for AppMsg<Ctx> {
    fn from(value: GetValidatorSet<Ctx>) -> Self {
        Self::GetValidatorSet(value)
    }
}

impl<Ctx: Context> From<Decided<Ctx>> for AppMsg<Ctx> {
    fn from(value: Decided<Ctx>) -> Self {
        Self::Decided(value)
    }
}

impl<Ctx: Context> From<GetDecidedValue<Ctx>> for AppMsg<Ctx> {
    fn from(value: GetDecidedValue<Ctx>) -> Self {
        Self::GetDecidedValue(value)
    }
}

impl<Ctx: Context> From<ProcessSyncedValue<Ctx>> for AppMsg<Ctx> {
    fn from(value: ProcessSyncedValue<Ctx>) -> Self {
        Self::ProcessSyncedValue(value)
    }
}

/// Notifies the application that consensus is ready.
///
/// The application MUST reply with a message to instruct
/// consensus to start at a given height.
#[derive_where(Debug)]
pub struct ConsensusReady<Ctx: Context> {
    /// Channel for sending back the height to start at
    /// and the validator set for that height
    pub reply: Reply<(Ctx::Height, Ctx::ValidatorSet)>,
}

/// Notifies the application that a new consensus round has begun.
#[derive_where(Debug)]
pub struct StartedRound<Ctx: Context> {
    /// Current consensus height
    pub height: Ctx::Height,
    /// Round that was just started
    pub round: Round,
    /// Proposer for that round
    pub proposer: Ctx::Address,
    /// Role that this node is playing in this round
    pub role: Role,
    /// Use this channel to send back any undecided values that were already seen for this round.
    /// This is needed when recovering from a crash.
    ///
    /// The application MUST reply immediately with the values it has, or with an empty vector.
    pub reply_value: Reply<Vec<ProposedValue<Ctx>>>,
}

/// Requests the application to build a value for consensus to propose.
///
/// The application MUST reply to this message with the requested value
/// within the specified timeout duration.
#[derive_where(Debug)]
pub struct GetValue<Ctx: Context> {
    /// Height for which the value is requested
    pub height: Ctx::Height,
    /// Round for which the value is requested
    pub round: Round,
    /// Maximum time allowed for the application to respond
    pub timeout: Duration,
    /// Channel for sending back the value just built to consensus
    pub reply: Reply<LocallyProposedValue<Ctx>>,
}

/// ExtendVote allows the application to extend the pre-commit vote with arbitrary data.
///
/// When consensus is preparing to send a pre-commit vote, it first calls `ExtendVote`.
/// The application then returns a blob of data called a vote extension.
/// This data is opaque to the consensus algorithm but can contain application-specific information.
/// The proposer of the next block will receive all vote extensions along with the commit certificate.
#[derive_where(Debug)]
pub struct ExtendVote<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value_id: ValueId<Ctx>,
    pub reply: Reply<Option<Ctx::Extension>>,
}

/// Verify a vote extension
///
/// If the vote extension is deemed invalid, the vote it was part of
/// will be discarded altogether.
#[derive_where(Debug)]
pub struct VerifyVoteExtension<Ctx: Context> {
    /// The height for which the vote is.
    pub height: Ctx::Height,
    /// The round for which the vote is.
    pub round: Round,
    /// The ID of the value that the vote extension is for.
    pub value_id: ValueId<Ctx>,
    /// The vote extension to verify.
    pub extension: Ctx::Extension,
    /// Use this channel to send the result of the verification.
    pub reply: Reply<Result<(), VoteExtensionError>>,
}

/// Requests the application to re-stream a proposal that it has already seen.
///
/// The application MUST re-publish again all the proposal parts pertaining
/// to that value by sending [`NetworkMsg::PublishProposalPart`] messages through
/// the [`Channels::network`] channel.
#[derive_where(Debug)]
pub struct RestreamProposal<Ctx: Context> {
    /// Height of the proposal
    pub height: Ctx::Height,
    /// Round of the proposal
    pub round: Round,
    /// Rround at which the proposal was locked on
    pub valid_round: Round,
    /// Address of the original proposer
    pub address: Ctx::Address,
    /// Unique identifier of the proposed value
    pub value_id: ValueId<Ctx>,
}

/// Requests the earliest height available in the history maintained by the application.
///
/// The application MUST respond with its earliest available height.
#[derive_where(Debug)]
pub struct GetHistoryMinHeight<Ctx: Context> {
    pub reply: Reply<Ctx::Height>,
}

/// Notifies the application that consensus has received a proposal part over the network.
///
/// If this part completes the full proposal, the application MUST respond
/// with the complete proposed value. Otherwise, it MUST respond with `None`.
#[derive_where(Debug)]
pub struct ReceivedProposalPart<Ctx: Context> {
    /// Peer whom the proposal part was received from
    pub from: PeerId,
    /// Received proposal part, together with its stream metadata
    pub part: StreamMessage<Ctx::ProposalPart>,
    /// Channel for returning the complete value if the proposal is now complete
    pub reply: Reply<Option<ProposedValue<Ctx>>>,
}

/// Requests the validator set for a specific height
#[derive_where(Debug)]
pub struct GetValidatorSet<Ctx: Context> {
    /// Height of the validator set to retrieve
    pub height: Ctx::Height,
    /// Channel for sending back the validator set
    pub reply: Reply<Option<Ctx::ValidatorSet>>,
}

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
#[derive_where(Debug)]
pub struct Decided<Ctx: Context> {
    /// The certificate for the decided value
    pub certificate: CommitCertificate<Ctx>,

    /// The vote extensions received for that height
    pub extensions: VoteExtensions<Ctx>,

    /// Channel for instructing consensus to start the next height, if desired
    pub reply: Reply<Next<Ctx>>,
}

/// Requests a previously decided value from the application's storage.
///
/// The application MUST respond with that value if available, or `None` otherwise.
#[derive_where(Debug)]
pub struct GetDecidedValue<Ctx: Context> {
    /// Height of the decided value to retrieve
    pub height: Ctx::Height,
    /// Channel for sending back the decided value
    pub reply: Reply<Option<RawDecidedValue<Ctx>>>,
}

/// Notifies the application that a value has been synced from the network.
/// This may happen when the node is catching up with the network.
///
/// If a value can be decoded from the bytes provided, then the application MUST reply
/// to this message with the decoded value. Otherwise, it MUST reply with `None`.
#[derive_where(Debug)]
pub struct ProcessSyncedValue<Ctx: Context> {
    /// Height of the synced value
    pub height: Ctx::Height,
    /// Round of the synced value
    pub round: Round,
    /// Address of the original proposer
    pub proposer: Ctx::Address,
    /// Raw encoded value data
    pub value_bytes: Bytes,
    /// Channel for sending back the proposed value, if successfully decoded
    /// or `None` if the value could not be decoded
    pub reply: Reply<Option<ProposedValue<Ctx>>>,
}

/// Messages sent from the application to consensus.
#[derive_where(Debug)]
pub enum ConsensusMsg<Ctx: Context> {
    /// Instructs consensus to start a new height with the given validator set.
    StartHeight(Ctx::Height, Ctx::ValidatorSet),

    /// Previousuly received value proposed by a validator
    ReceivedProposedValue(ProposedValue<Ctx>, ValueOrigin),

    /// Instructs consensus to restart at a given height with the given validator set.
    RestartHeight(Ctx::Height, Ctx::ValidatorSet),
}

impl<Ctx: Context> From<ConsensusMsg<Ctx>> for ConsensusActorMsg<Ctx> {
    fn from(msg: ConsensusMsg<Ctx>) -> ConsensusActorMsg<Ctx> {
        match msg {
            ConsensusMsg::StartHeight(height, validator_set) => {
                ConsensusActorMsg::StartHeight(height, validator_set)
            }
            ConsensusMsg::ReceivedProposedValue(value, origin) => {
                ConsensusActorMsg::ReceivedProposedValue(value, origin)
            }
            ConsensusMsg::RestartHeight(height, validator_set) => {
                ConsensusActorMsg::RestartHeight(height, validator_set)
            }
        }
    }
}

/// Messages sent from the application to the networking layer.
#[derive_where(Debug)]
pub enum NetworkMsg<Ctx: Context> {
    /// Publish a proposal part to the network, within a stream.
    PublishProposalPart(StreamMessage<Ctx::ProposalPart>),
}

impl<Ctx: Context> From<NetworkMsg<Ctx>> for NetworkActorMsg<Ctx> {
    fn from(msg: NetworkMsg<Ctx>) -> NetworkActorMsg<Ctx> {
        match msg {
            NetworkMsg::PublishProposalPart(part) => NetworkActorMsg::PublishProposalPart(part),
        }
    }
}
