use std::time::Duration;

use bytes::Bytes;
use derive_where::derive_where;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::error;

use malachitebft_app::consensus::Role;
use malachitebft_app::consensus::VoteExtensionError;
use malachitebft_app::types::core::ValueOrigin;
use malachitebft_engine::consensus::state_dump::StateDump;
use malachitebft_engine::consensus::Msg as ConsensusActorMsg;
use malachitebft_engine::host::Next;
use malachitebft_engine::network::Msg as NetworkActorMsg;
use malachitebft_engine::util::events::TxEvent;

use crate::app::types::core::{CommitCertificate, Context, Round, ValueId, VoteExtensions};
use crate::app::types::streaming::StreamMessage;
use crate::app::types::sync::RawDecidedValue;
use crate::app::types::{LocallyProposedValue, PeerId, ProposedValue};

pub type Reply<T> = oneshot::Sender<T>;

/// Errors that can occur when sending a request to consensus or receiving its response.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Error)]
pub enum ConsensusRequestError {
    /// The request channel is closed (typically because consensus has stopped)
    #[error("The request channel is closed")]
    Closed,
    /// The request channel is full (there are more requests than consensus can process)
    #[error("The request channel is full")]
    Full,
    /// Failed to receive the response (consensus went down before sending a reply or something else went wrong)
    #[error("Failed to receive the response")]
    Recv,
}

impl<T> From<mpsc::error::TrySendError<T>> for ConsensusRequestError {
    fn from(err: mpsc::error::TrySendError<T>) -> Self {
        match err {
            mpsc::error::TrySendError::Closed(_) => Self::Closed,
            mpsc::error::TrySendError::Full(_) => Self::Full,
        }
    }
}

impl From<oneshot::error::RecvError> for ConsensusRequestError {
    fn from(_: oneshot::error::RecvError) -> Self {
        Self::Recv
    }
}

/// Represents requests that can be sent to the consensus engine by the application.
///
/// Each variant corresponds to a specific operation or query that the consensus engine can perform.
/// To send a request, use the `requests` channel provided in [`Channels`].
/// Responses are delivered via oneshot channels included in the request variants.
///
/// ## Example
///
/// ```rust,ignore
/// pub async fn run(state: &mut State, channels: &mut Channels<TestContext>) -> eyre::Result<()> {
///     // If the MALACHITE_MONITOR_STATE env var is set, start monitoring the consensus state
///     if std::env::var("MALACHITE_MONITOR_STATE").is_ok() {
///         monitor_state(channels.requests.clone());
///     }
///
///     // ...
/// }
///
/// /// Periodically request a state dump from consensus and print it to the console
/// fn monitor_state(tx_request: mpsc::Sender<ConsensusRequest<TestContext>>) {
///     tokio::spawn(async move {
///         loop {
///             match ConsensusRequest::dump_state(&tx_request).await {
///                 Ok(dump) => {
///                     tracing::debug!("State dump: {dump:#?}");
///                 }
///                 Err(ConsensusRequestError::Recv) => {
///                     tracing::error!("Failed to receive state dump from consensus");
///                 }
///                 Err(ConsensusRequestError::Full) => {
///                     tracing::error!("Consensus request channel full");
///                 }
///                 Err(ConsensusRequestError::Closed) => {
///                     tracing::error!("Consensus request channel closed");
///                 }
///             }
///
///             sleep(Duration::from_secs(1)).await;
///         }
///     });
/// }
/// ```
pub enum ConsensusRequest<Ctx: Context> {
    /// Request a state dump from consensus
    DumpState(Reply<StateDump<Ctx>>),
}

impl<Ctx: Context> ConsensusRequest<Ctx> {
    /// Request a state dump from consensus.
    ///
    /// If the request fails, `None` is returned.
    pub async fn dump_state(
        tx_request: &mpsc::Sender<ConsensusRequest<Ctx>>,
    ) -> Result<StateDump<Ctx>, ConsensusRequestError> {
        let (tx, rx) = oneshot::channel();

        tx_request
            .try_send(Self::DumpState(tx))
            .inspect_err(|e| error!("Failed to send DumpState request to consensus: {e}"))?;

        let dump = rx
            .await
            .inspect_err(|e| error!("Failed to receive DumpState response from consensus: {e}"))?;

        Ok(dump)
    }
}

/// Channels created for application consumption
pub struct Channels<Ctx: Context> {
    /// Channel for receiving messages from consensus
    pub consensus: mpsc::Receiver<AppMsg<Ctx>>,
    /// Channel for sending messages to the networking layer
    pub network: mpsc::Sender<NetworkMsg<Ctx>>,
    /// Receiver of events, call `subscribe` to receive them
    pub events: TxEvent<Ctx>,
    /// Channel for sending requests to consensus
    pub requests: mpsc::Sender<ConsensusRequest<Ctx>>,
}

/// Messages sent from consensus to the application.
#[derive_where(Debug)]
pub enum AppMsg<Ctx: Context> {
    /// Notifies the application that consensus is ready.
    ///
    /// The application MUST reply with a message to instruct
    /// consensus to start at a given height.
    ConsensusReady {
        /// Channel for sending back the height to start at
        /// and the validator set for that height
        reply: Reply<(Ctx::Height, Ctx::ValidatorSet)>,
    },

    /// Notifies the application that a new consensus round has begun.
    StartedRound {
        /// Current consensus height
        height: Ctx::Height,
        /// Round that was just started
        round: Round,
        /// Proposer for that round
        proposer: Ctx::Address,
        /// Role that this node is playing in this round
        role: Role,
        /// Use this channel to send back any undecided values that were already seen for this round.
        /// This is needed when recovering from a crash.
        ///
        /// The application MUST reply immediately with the values it has, or with an empty vector.
        reply_value: Reply<Vec<ProposedValue<Ctx>>>,
    },

    /// Requests the application to build a value for consensus to propose.
    ///
    /// The application MUST reply to this message with the requested value
    /// within the specified timeout duration.
    GetValue {
        /// Height for which the value is requested
        height: Ctx::Height,
        /// Round for which the value is requested
        round: Round,
        /// Maximum time allowed for the application to respond
        timeout: Duration,
        /// Channel for sending back the value just built to consensus
        reply: Reply<LocallyProposedValue<Ctx>>,
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
        reply: Reply<Option<Ctx::Extension>>,
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
        /// Use this channel to send the result of the verification.
        reply: Reply<Result<(), VoteExtensionError>>,
    },

    /// Requests the application to re-stream a proposal that it has already seen.
    ///
    /// The application MUST re-publish again all the proposal parts pertaining
    /// to that value by sending [`NetworkMsg::PublishProposalPart`] messages through
    /// the [`Channels::network`] channel.
    RestreamProposal {
        /// Height of the proposal
        height: Ctx::Height,
        /// Round of the proposal
        round: Round,
        /// Rround at which the proposal was locked on
        valid_round: Round,
        /// Address of the original proposer
        address: Ctx::Address,
        /// Unique identifier of the proposed value
        value_id: ValueId<Ctx>,
    },

    /// Requests the earliest height available in the history maintained by the application.
    ///
    /// The application MUST respond with its earliest available height.
    GetHistoryMinHeight { reply: Reply<Ctx::Height> },

    /// Notifies the application that consensus has received a proposal part over the network.
    ///
    /// If this part completes the full proposal, the application MUST respond
    /// with the complete proposed value. Otherwise, it MUST respond with `None`.
    ReceivedProposalPart {
        /// Peer whom the proposal part was received from
        from: PeerId,
        /// Received proposal part, together with its stream metadata
        part: StreamMessage<Ctx::ProposalPart>,
        /// Channel for returning the complete value if the proposal is now complete
        reply: Reply<Option<ProposedValue<Ctx>>>,
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
        /// The certificate for the decided value
        certificate: CommitCertificate<Ctx>,

        /// The vote extensions received for that height
        extensions: VoteExtensions<Ctx>,

        /// Channel for instructing consensus to start the next height, if desired
        reply: Reply<Next<Ctx>>,
    },

    /// Requests a previously decided value from the application's storage.
    ///
    /// The application MUST respond with that value if available, or `None` otherwise.
    GetDecidedValue {
        /// Height of the decided value to retrieve
        height: Ctx::Height,
        /// Channel for sending back the decided value
        reply: Reply<Option<RawDecidedValue<Ctx>>>,
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
        reply: Reply<Option<ProposedValue<Ctx>>>,
    },
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
