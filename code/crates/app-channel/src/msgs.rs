use std::time::Duration;

use bytes::Bytes;
use derive_where::derive_where;
use malachitebft_app::types::core::ValueOrigin;
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
    /// The application MAY reply with a message to instruct
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
        /// Channel for sending back previously received undecided values to consensus
        reply_value: Reply<Vec<ProposedValue<Ctx>>>,
    },

    /// Requests the application to build a value for consensus to run on.
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
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        extension: Ctx::Extension,
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

    /// Requests the validator set for a specific height
    GetValidatorSet {
        /// Height of the validator set to retrieve
        height: Ctx::Height,
        /// Channel for sending back the validator set
        reply: Reply<Option<Ctx::ValidatorSet>>,
    },

    /// Notifies the application that consensus has decided on a value.
    ///
    /// This message includes a commit certificate containing the ID of
    /// the value that was decided on, the height and round at which it was decided,
    /// and the aggregated signatures of the validators that committed to it.
    /// It also includes to the vote extensions received for that height.
    ///
    /// In response to this message, the application MAY send a [`ConsensusMsg::StartHeight`]
    /// message back to consensus, instructing it to start the next height.
    Decided {
        /// The certificate for the decided value
        certificate: CommitCertificate<Ctx>,

        /// The vote extensions received for that height
        extensions: VoteExtensions<Ctx>,

        /// Channel for instructing consensus to start the next height, if desired
        reply: Reply<ConsensusMsg<Ctx>>,
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
    /// to this message with the decoded value.
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
        reply: Reply<ProposedValue<Ctx>>,
    },

    /// Notifies the application that a peer has joined our local view of the network.
    ///
    /// In a gossip network, there is no guarantee that we will ever see all peers,
    /// as we are typically only connected to a subset of the network (i.e. in our mesh).
    PeerJoined {
        /// The ID of the peer that joined
        peer_id: PeerId,
    },

    /// Notifies the application that a peer has left our local view of the network.
    ///
    /// In a gossip network, there is no guarantee that this means that this peer
    /// has left the whole network altogether, just that it is not part of the subset
    /// of the network that we are connected to (i.e. our mesh).
    PeerLeft {
        /// The ID of the peer that left
        peer_id: PeerId,
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
