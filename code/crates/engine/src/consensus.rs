use core::fmt;
use std::collections::BTreeSet;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use async_recursion::async_recursion;
use async_trait::async_trait;
use derive_where::derive_where;
use eyre::eyre;
use itertools::Itertools;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use tokio::time::Instant;
use tracing::{debug, error, error_span, info, warn};

use malachitebft_codec as codec;
use malachitebft_config::ConsensusConfig;
use malachitebft_core_consensus::{
    Effect, LivenessMsg, PeerId, Resumable, Resume, SignedConsensusMsg, VoteExtensionError,
    VoteExtensionVerdicts,
};
use malachitebft_core_types::{
    CertificateError, Context, ExtendedCommitCertificate, Proposal, Round, Timeout, TimeoutKind,
    Timeouts, ValidatorProof, ValidatorSet, Value, ValueId, ValueOrigin,
    ValueResponse as CoreValueResponse, Vote, VoteExtensionPolicy, VoteExtensionScope,
};
use malachitebft_metrics::{DropReason, DropReasonLabel, Metrics};
use malachitebft_signing::{Signer, Verifier, VerifierExt};
use malachitebft_sync::{HeightStartType, OutboundRequestId};

use crate::host::{
    HeightParams, HostMsg, HostRef, LocallyProposedValue, Next, ProposedValue, SyncedValueOutcome,
};
use crate::network::{NetworkEvent, NetworkMsg, NetworkRef};
use crate::node::NodeRef;
use crate::sync::Msg as SyncMsg;
use crate::util::events::{Event, TxEvent};
use crate::util::failure::{hang_on_safety_failure, stop_on_failure};
use crate::util::msg_buffer::{BufferFull, MessageBuffer};
use crate::util::output_port::OutputPort;
use crate::util::ractor::{cast_and_handle, ReplyDropped};
use crate::util::streaming::StreamMessage;
use crate::util::timers::{TimeoutElapsed, TimerScheduler};
use crate::wal::{Msg as WalMsg, WalRef};

pub use malachitebft_core_consensus::Error as ConsensusError;
pub use malachitebft_core_consensus::Params as ConsensusParams;
pub use malachitebft_core_consensus::State as ConsensusState;

pub mod state_dump;
use state_dump::StateDump;

/// Failure reported by the runtime `wal_append` / `wal_flush` helpers.
///
/// Safety-critical: a message signed and broadcast without a matching durable
/// WAL entry lets a restart replay an empty WAL and sign a conflicting message
/// for the same `(height, round, step)` — a slashable equivocation. Call sites
/// therefore route every variant through
/// [`crate::util::failure::hang_on_safety_failure`].
#[derive(Debug)]
enum WalFailure {
    /// WAL actor replied with an error while writing the entry.
    Write(eyre::Report),

    /// WAL actor replied with an error while syncing to disk.
    Flush(eyre::Report),

    /// Request to the WAL actor failed to deliver or reply; outcome unknown.
    Transport(String),
}

impl fmt::Display for WalFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WalFailure::Write(e) => write!(f, "WAL write error: {e}"),
            WalFailure::Flush(e) => write!(f, "WAL flush error: {e}"),
            WalFailure::Transport(e) => write!(f, "WAL actor transport error: {e}"),
        }
    }
}

/// Codec for consensus messages.
///
/// This trait is automatically implemented for any type that implements:
/// - [`codec::Codec<Ctx::ProposalPart>`]
/// - [`codec::Codec<SignedConsensusMsg<Ctx>>`]
/// - [`codec::Codec<PolkaCertificate<Ctx>>`]
/// - [`codec::Codec<StreamMessage<Ctx::ProposalPart>>`]
/// - [`codec::Codec<ValidatorProof<Ctx>>`]
pub trait ConsensusCodec<Ctx>
where
    Ctx: Context,
    Self: codec::Codec<Ctx::ProposalPart>,
    Self: codec::Codec<SignedConsensusMsg<Ctx>>,
    Self: codec::Codec<LivenessMsg<Ctx>>,
    Self: codec::Codec<StreamMessage<Ctx::ProposalPart>>,
    Self: codec::Codec<ValidatorProof<Ctx>>,
{
}

impl<Ctx, Codec> ConsensusCodec<Ctx> for Codec
where
    Ctx: Context,
    Self: codec::Codec<Ctx::ProposalPart>,
    Self: codec::Codec<SignedConsensusMsg<Ctx>>,
    Self: codec::Codec<LivenessMsg<Ctx>>,
    Self: codec::Codec<StreamMessage<Ctx::ProposalPart>>,
    Self: codec::Codec<ValidatorProof<Ctx>>,
{
}

pub type ConsensusRef<Ctx> = ActorRef<Msg<Ctx>>;

pub struct Consensus<Ctx>
where
    Ctx: Context,
{
    ctx: Ctx,
    params: ConsensusParams<Ctx>,
    consensus_config: ConsensusConfig,
    verifier: Box<dyn Verifier<Ctx>>,
    signer: Option<Box<dyn Signer<Ctx>>>,
    network: NetworkRef<Ctx>,
    host: HostRef<Ctx>,
    wal: WalRef<Ctx>,
    sync: Arc<OutputPort<SyncMsg<Ctx>>>,
    metrics: Metrics,
    tx_event: TxEvent<Ctx>,
    node: NodeRef,
    span: tracing::Span,
}

pub type ConsensusMsg<Ctx> = Msg<Ctx>;

/// Generation stamped on [`Msg::DecisionCommitted`]. Bumped on
/// [`Msg::RestartHeight`] so a late host ack cannot advance the value-sync tip.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitGeneration(u64);

impl CommitGeneration {
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

impl fmt::Display for CommitGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive_where(Debug)]
pub enum Msg<Ctx: Context> {
    /// Start consensus for the given height and provided parameters.
    StartHeight(Ctx::Height, HeightParams<Ctx>),

    /// Received an event from the gossip layer
    NetworkEvent(NetworkEvent<Ctx>),

    /// A timeout has elapsed
    TimeoutElapsed(TimeoutElapsed<Timeout>),

    /// The proposal builder has built a value and can be used in a new proposal consensus message
    ProposeValue(LocallyProposedValue<Ctx>),

    /// Received and assembled the full value proposed by a validator
    ReceivedProposedValue(ProposedValue<Ctx>, ValueOrigin),

    /// Process a sync response from the request identified by the first field.
    ProcessSyncResponse(OutboundRequestId, CoreValueResponse<Ctx>),

    /// Instructs consensus to restart at a given height with the provided parameters.
    ///
    /// On this input consensus resets the Write-Ahead Log.
    ///
    /// # Warning
    /// This operation should be used with extreme caution as it can lead to safety violations:
    /// 1. The application must clean all state associated with the height for which commit has failed
    /// 2. Since consensus resets its write-ahead log, the node may equivocate on proposals and votes
    ///    for the restarted height, potentially violating protocol safety
    RestartHeight(Ctx::Height, HeightParams<Ctx>),

    /// The application has confirmed that the decision has been committed.
    /// This triggers notifying the sync actor about the decided height,
    /// unless a `RestartHeight` has invalidated the acknowledgement's generation.
    DecisionCommitted(Ctx::Height, CommitGeneration),

    /// Request to dump the current consensus state
    DumpState(RpcReplyPort<Option<StateDump<Ctx>>>),
}

impl<Ctx: Context> fmt::Display for Msg<Ctx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Msg::StartHeight(height, params) => {
                write!(f, "StartHeight(height={height} params={params:?})")
            }
            Msg::NetworkEvent(event) => match event {
                NetworkEvent::Proposal(_, proposal) => write!(
                    f,
                    "NetworkEvent(Proposal height={} round={})",
                    proposal.height(),
                    proposal.round()
                ),
                NetworkEvent::ProposalPart(_, _, part) => {
                    write!(f, "NetworkEvent(ProposalPart sequence={})", part.sequence)
                }
                NetworkEvent::Vote(_, vote) => write!(
                    f,
                    "NetworkEvent(Vote height={} round={})",
                    vote.height(),
                    vote.round()
                ),
                _ => write!(f, "NetworkEvent"),
            },
            Msg::TimeoutElapsed(timeout) => write!(f, "TimeoutElapsed({})", timeout.display_key()),
            Msg::ProposeValue(value) => write!(
                f,
                "ProposeValue(height={} round={})",
                value.height, value.round
            ),
            Msg::ReceivedProposedValue(value, origin) => write!(
                f,
                "ReceivedProposedValue(height={} round={} origin={origin:?})",
                value.height, value.round
            ),
            Msg::ProcessSyncResponse(request_id, response) => {
                write!(
                    f,
                    "ProcessSyncResponse(request_id={request_id} peer={} height={} value={})",
                    response.peer, response.certificate.height, response.certificate.value_id,
                )
            }
            Msg::RestartHeight(height, params) => {
                write!(f, "RestartHeight(height={height} params={params:?})")
            }
            Msg::DecisionCommitted(height, generation) => {
                write!(
                    f,
                    "DecisionCommitted(height={height} generation={generation})"
                )
            }
            Msg::DumpState(_) => write!(f, "DumpState"),
        }
    }
}

impl<Ctx: Context> From<NetworkEvent<Ctx>> for Msg<Ctx> {
    fn from(event: NetworkEvent<Ctx>) -> Self {
        Self::NetworkEvent(event)
    }
}

type ConsensusInput<Ctx> = malachitebft_core_consensus::Input<Ctx>;

impl<Ctx: Context> From<TimeoutElapsed<Timeout>> for Msg<Ctx> {
    fn from(msg: TimeoutElapsed<Timeout>) -> Self {
        Msg::TimeoutElapsed(msg)
    }
}

type Timers = TimerScheduler<Timeout>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Phase {
    Unstarted,
    Ready,
    Running,
    Recovering,
}

/// Maximum number of messages to buffer while consensus is
/// not in the `Running` phase
const MAX_BUFFER_SIZE: usize = 1024;

pub struct State<Ctx: Context> {
    /// Scheduler for timers
    timers: Timers,

    /// Timeouts for various consensus steps
    timeouts: Ctx::Timeouts,

    /// The state of the consensus state machine,
    /// or `None` if consensus has not been started yet.
    consensus: Option<ConsensusState<Ctx>>,

    /// The set of peers we are connected to.
    connected_peers: BTreeSet<PeerId>,

    /// The current phase
    phase: Phase,

    /// Whether this node is in the validator set for the current height.
    /// Non-validators skip WAL writes since they have no equivocation risk.
    is_validator: bool,

    /// A buffer of messages that were received while
    /// consensus was not in the `Running` phase
    msg_buffer: MessageBuffer<Msg<Ctx>>,

    /// Stamped on in-flight `DecisionCommitted` acknowledgements.
    commit_generation: CommitGeneration,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn height(&self) -> Ctx::Height {
        self.consensus
            .as_ref()
            .map(|c| c.height())
            .unwrap_or_default()
    }

    pub fn round(&self) -> Round {
        self.consensus
            .as_ref()
            .map(|c| c.round())
            .unwrap_or(Round::Nil)
    }

    fn set_phase(&mut self, phase: Phase) {
        if self.phase != phase {
            info!(prev = ?self.phase, new = ?phase, "Phase transition");
            self.phase = phase;
        }
    }
}

struct HandlerState<'a, Ctx: Context> {
    phase: Phase,
    is_validator: bool,
    timers: &'a mut Timers,
    timeouts: Ctx::Timeouts,
    sync_request_id: Option<OutboundRequestId>,
    commit_generation: CommitGeneration,
}

impl<Ctx> Consensus<Ctx>
where
    Ctx: Context,
{
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        ctx: Ctx,
        params: ConsensusParams<Ctx>,
        consensus_config: ConsensusConfig,
        verifier: Box<dyn Verifier<Ctx>>,
        signer: Option<Box<dyn Signer<Ctx>>>,
        network: NetworkRef<Ctx>,
        host: HostRef<Ctx>,
        wal: WalRef<Ctx>,
        sync: Arc<OutputPort<SyncMsg<Ctx>>>,
        metrics: Metrics,
        tx_event: TxEvent<Ctx>,
        node: NodeRef,
        span: tracing::Span,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let actor = Self {
            ctx,
            params,
            consensus_config,
            verifier,
            signer,
            network,
            host,
            wal,
            sync,
            metrics,
            tx_event,
            node,
            span,
        };

        let (actor_ref, _) = Actor::spawn(None, actor, ()).await?;
        Ok(actor_ref)
    }

    fn signer(&self) -> &dyn Signer<Ctx> {
        self.signer.as_deref().expect(
            "BUG: signing effect produced but no signer configured; \
             this node should not be a validator",
        )
    }

    async fn process_input(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        input: ConsensusInput<Ctx>,
    ) -> Result<(), ConsensusError<Ctx>> {
        self.process_input_with_sync_request(myself, state, input, None)
            .await
    }

    async fn process_sync_response(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        request_id: OutboundRequestId,
        response: CoreValueResponse<Ctx>,
    ) -> Result<(), ConsensusError<Ctx>> {
        self.process_input_with_sync_request(
            myself,
            state,
            ConsensusInput::SyncValueResponse(response),
            Some(request_id),
        )
        .await
    }

    async fn process_input_with_sync_request(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        input: ConsensusInput<Ctx>,
        sync_request_id: Option<OutboundRequestId>,
    ) -> Result<(), ConsensusError<Ctx>> {
        malachitebft_core_consensus::process!(
            input: input,
            state: state.consensus.as_mut().expect("Consensus not started"),
            metrics: &self.metrics,
            with: effect => {
                let handler_state = HandlerState {
                    phase: state.phase,
                    is_validator: state.is_validator,
                    timers: &mut state.timers,
                    timeouts: state.timeouts,
                    sync_request_id: sync_request_id.clone(),
                    commit_generation: state.commit_generation,
                };

                self.handle_effect(myself, handler_state, effect).await
            }
        )
    }

    #[async_recursion]
    async fn process_buffered_msgs(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        is_restart: bool,
    ) -> Result<(), ActorProcessingErr> {
        if state.msg_buffer.is_empty() {
            return Ok(());
        }

        if is_restart {
            // The restarted height supersedes anything buffered for it.
            warn!(count = %state.msg_buffer.len(), "Discarding buffered messages on restart");

            while let Some(msg) = state.msg_buffer.pop() {
                self.report_dropped_msg(&msg, DropReason::Restart);
            }

            return Ok(());
        }

        info!(count = %state.msg_buffer.len(), "Replaying buffered messages");

        while let Some(msg) = state.msg_buffer.pop() {
            debug!("Replaying buffered message: {msg}");

            self.handle_msg(myself.clone(), state, msg).await?;
        }

        Ok(())
    }

    /// Meter a message that was dropped from the buffer instead of being
    /// handed to consensus.
    fn report_dropped_msg(&self, msg: &Msg<Ctx>, reason: DropReason) {
        self.metrics
            .dropped_buffered_messages
            .get_or_create(&DropReasonLabel::new(reason))
            .inc();

        // A dropped sync message is not recovered: the sync actor keeps its
        // range reserved until consensus advances past it.
        if is_sync_application_msg(msg) {
            error!(
                ?reason,
                "Dropped sync message, catch-up cannot make progress: {msg}"
            );
        }
    }

    async fn handle_msg(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        msg: Msg<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        let is_restart = matches!(msg, Msg::RestartHeight(_, _));

        match msg {
            Msg::StartHeight(height, params) | Msg::RestartHeight(height, params) => {
                // Check that the validator set is provided and that it is not empty
                if params.validator_set.count() == 0 {
                    return Err(eyre!("Validator set for height {height} is empty").into());
                }

                if is_restart {
                    state.commit_generation = state.commit_generation.next();
                }

                // Initialize consensus state if this is the first height we start
                if state.consensus.is_none() {
                    state.consensus = Some(ConsensusState::new(
                        self.ctx.clone(),
                        height,
                        params.validator_set.clone(),
                        self.params.clone(),
                        self.consensus_config.queue_capacity,
                        self.consensus_config.queue_per_height_capacity,
                    ));
                }

                self.tx_event
                    .send(|| Event::StartedHeight(height, is_restart));

                // Determine if this node is an active validator for this height.
                // Mirrors ConsensusState::is_active_validator(): both `enabled` and
                // validator set membership must hold.
                state.is_validator = self.params.enabled
                    && params
                        .validator_set
                        .get_by_address(&self.params.address)
                        .is_some();

                // Push validator set to network layer
                if let Err(e) = self
                    .network
                    .cast(NetworkMsg::UpdateValidatorSet(params.validator_set.clone()))
                {
                    error!(%height, "Error pushing validator set to network layer: {e}");
                }

                // Fetch entries from the WAL or reset the WAL if this is a restart.
                // Non-validators skip WAL recovery and reset any stale entries.
                //
                // Startup-path WAL errors take the liveness path: no signing has
                // happened yet this run, so an orchestrator restart is safe.
                let wal_entries = if is_restart {
                    stop_on_failure(self.wal_reset(height), |e| {
                        format!("wal_reset at height {height} failed: {e}")
                    })
                    .await?;

                    vec![]
                } else if !state.is_validator {
                    stop_on_failure(self.wal_reset(height), |e| {
                        format!("wal_reset (non-validator) at height {height} failed: {e}")
                    })
                    .await?;

                    vec![]
                } else {
                    stop_on_failure(self.wal_fetch(height), |e| {
                        format!("wal_fetch at height {height} failed: {e}")
                    })
                    .await?
                };

                // Update the timeouts
                state.timeouts = params.timeouts;

                // Start consensus for the given height
                stop_on_failure(
                    self.process_input(
                        &myself,
                        state,
                        ConsensusInput::StartHeight(
                            height,
                            params.validator_set,
                            is_restart,
                            params.target_time,
                            params.vote_extension_policy,
                        ),
                    ),
                    |e| format!("starting height {height} failed: {e}"),
                )
                .await?;

                if !wal_entries.is_empty() {
                    state.set_phase(Phase::Recovering);

                    self.wal_replay(&myself, state, height, wal_entries).await;
                }

                // Set the phase to `Running` now that we have replayed the WAL
                state.set_phase(Phase::Running);

                // Notify the sync actor that we have started a new height.
                // NOTE: SyncMsg::Decided is sent separately via Msg::DecisionCommitted,
                // which fires when the app confirms the decision commit (after Effect::Decide).
                let start_type = HeightStartType::from_is_restart(is_restart);
                self.sync.send(SyncMsg::StartedHeight(height, start_type));

                // Process any buffered messages, now that we are in the `Running` phase
                self.process_buffered_msgs(&myself, state, is_restart)
                    .await?;

                Ok(())
            }

            Msg::ProposeValue(value) => {
                stop_on_failure(
                    self.process_input(&myself, state, ConsensusInput::Propose(value.clone())),
                    |e| {
                        format!(
                            "processing ProposeValue at height {} round {} failed: {e}",
                            value.height, value.round
                        )
                    },
                )
                .await?;

                self.tx_event.send(|| Event::ProposedValue(value));

                Ok(())
            }

            Msg::NetworkEvent(event) => {
                match event {
                    NetworkEvent::Listening(address) => {
                        info!(%address, "Listening");

                        if state.phase == Phase::Unstarted {
                            state.set_phase(Phase::Ready);

                            self.host.call_and_forward(
                                |reply_to| HostMsg::ConsensusReady { reply_to },
                                &myself,
                                |(height, params)| ConsensusMsg::StartHeight(height, params),
                                None,
                            )?;
                        }
                    }

                    NetworkEvent::PeerConnected(peer_id) => {
                        if !state.connected_peers.insert(peer_id) {
                            // We already saw that peer, ignoring...
                            return Ok(());
                        }

                        info!(%peer_id, total = %state.connected_peers.len(), "Connected to peer");

                        self.metrics.connected_peers.inc();
                    }

                    NetworkEvent::PeerDisconnected(peer_id) => {
                        info!(%peer_id, "Disconnected from peer");

                        if state.connected_peers.remove(&peer_id) {
                            self.metrics.connected_peers.dec();
                        }
                    }

                    NetworkEvent::Vote(from, vote) => {
                        self.tx_event
                            .send(|| Event::Received(SignedConsensusMsg::Vote(vote.clone())));

                        stop_on_failure(
                            self.process_input(&myself, state, ConsensusInput::Vote(vote)),
                            |e| format!("processing vote from {from} failed: {e}"),
                        )
                        .await?;
                    }

                    NetworkEvent::Proposal(from, proposal) => {
                        self.tx_event.send(|| {
                            Event::Received(SignedConsensusMsg::Proposal(proposal.clone()))
                        });

                        stop_on_failure(
                            self.process_input(&myself, state, ConsensusInput::Proposal(proposal)),
                            |e| format!("processing proposal from {from} failed: {e}"),
                        )
                        .await?;
                    }

                    NetworkEvent::PolkaCertificate(from, certificate) => {
                        stop_on_failure(
                            self.process_input(
                                &myself,
                                state,
                                ConsensusInput::PolkaCertificate(certificate),
                            ),
                            |e| format!("processing polka certificate from {from} failed: {e}"),
                        )
                        .await?;
                    }

                    NetworkEvent::RoundCertificate(from, certificate) => {
                        stop_on_failure(
                            self.process_input(
                                &myself,
                                state,
                                ConsensusInput::RoundCertificate(certificate),
                            ),
                            |e| format!("processing round certificate from {from} failed: {e}"),
                        )
                        .await?;
                    }

                    NetworkEvent::ProposalPart(from, published_by, part) => {
                        if self.params.value_payload.proposal_only() {
                            error!(%from, "Properly configured peer should never send proposal part messages in Proposal mode");
                            return Ok(());
                        }

                        self.host
                            .call_and_forward(
                                |reply_to| HostMsg::ReceivedProposalPart {
                                    from,
                                    published_by,
                                    part,
                                    reply_to,
                                },
                                &myself,
                                move |value| {
                                    Msg::ReceivedProposedValue(value, ValueOrigin::Consensus)
                                },
                                None,
                            )
                            .map_err(|e| {
                                eyre!("Error when forwarding proposal parts to host: {e}")
                            })?;
                    }

                    NetworkEvent::ValidatorProofReceived { peer_id, proof } => {
                        use malachitebft_network::validator_proof::ProofVerificationResult;

                        // Note: peer_id match is already verified in network layer

                        // Verify signature using public_key in proof
                        let verification = self.verifier.verify_validator_proof(&proof).await;

                        let (result, public_key_bytes) = match verification {
                            Ok(v) if v.is_valid() => {
                                debug!(
                                    %peer_id,
                                    public_key = %hex::encode(&proof.public_key),
                                    "Valid validator proof received"
                                );
                                (
                                    ProofVerificationResult::Valid,
                                    Some(proof.public_key.clone()),
                                )
                            }
                            Ok(_) => {
                                warn!(%peer_id, "Invalid validator proof signature");
                                (ProofVerificationResult::Invalid, None)
                            }
                            Err(e) => {
                                warn!(%peer_id, "Error verifying validator proof: {e}");
                                (ProofVerificationResult::Invalid, None)
                            }
                        };

                        // Send verification result to network layer
                        if let Err(e) = self.network.cast(NetworkMsg::ValidatorProofVerified {
                            peer_id,
                            result,
                            public_key: public_key_bytes,
                        }) {
                            error!(%peer_id, ?result, "Error sending validator proof result: {e}");
                        }
                    }

                    _ => {}
                }

                Ok(())
            }

            Msg::TimeoutElapsed(elapsed) => {
                let Some(timeout) = state.timers.intercept_timer_msg(elapsed) else {
                    // Timer was cancelled or already processed, ignore
                    return Ok(());
                };

                stop_on_failure(self.timeout_elapsed(&myself, state, timeout), |e| {
                    format!("processing TimeoutElapsed message failed: {e}")
                })
                .await?;

                Ok(())
            }

            Msg::ReceivedProposedValue(value, origin) => {
                self.tx_event
                    .send(|| Event::ReceivedProposedValue(value.clone(), origin));

                stop_on_failure(
                    self.process_input(
                        &myself,
                        state,
                        ConsensusInput::ProposedValue(value, origin),
                    ),
                    |e| format!("processing ReceivedProposedValue message failed: {e}"),
                )
                .await?;

                Ok(())
            }

            Msg::ProcessSyncResponse(request_id, response) => {
                let height = response.certificate.height;
                let round = response.certificate.round;
                let value = response.certificate.value_id.clone();
                let peer = response.peer;

                debug!(
                    %height, %round, %value, %peer,
                    "Processing sync response"
                );

                stop_on_failure(
                    self.process_sync_response(&myself, state, request_id, response),
                    |e| {
                        format!(
                            "processing sync response from {peer} at height {height} round {round} value {value} failed: {e}"
                        )
                    },
                )
                .await?;

                Ok(())
            }

            Msg::DecisionCommitted(height, generation) => {
                if generation != state.commit_generation {
                    debug!(
                        %height,
                        %generation,
                        current_generation = %state.commit_generation,
                        "Ignoring stale DecisionCommitted after height restart"
                    );
                    return Ok(());
                }

                // The application has confirmed that the decision has been committed.
                // Notify the sync actor so it can advertise this height to peers.
                self.sync.send(SyncMsg::Decided(height));

                Ok(())
            }

            Msg::DumpState(reply_to) => {
                let state_dump = if let Some(consensus) = &state.consensus {
                    info!(
                        height = %consensus.height(),
                        round  = %consensus.round(),
                        "Dumping consensus state"
                    );

                    Some(StateDump::new(consensus))
                } else {
                    info!("Dumping consensus state: not started");
                    None
                };

                if let Err(e) = reply_to.send(state_dump) {
                    error!("Failed to reply with state dump: {e}");
                }

                Ok(())
            }
        }
    }

    async fn timeout_elapsed(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        timeout: Timeout,
    ) -> Result<(), ConsensusError<Ctx>> {
        // Make sure the associated timer is cancelled
        state.timers.cancel(&timeout);

        // Print debug information if the timeout is for a prevote or precommit
        if matches!(
            timeout.kind,
            TimeoutKind::Prevote | TimeoutKind::Precommit | TimeoutKind::Rebroadcast
        ) {
            info!(step = ?timeout.kind, "Timeout elapsed");

            state.consensus.as_ref().inspect(|consensus| {
                consensus.print_state();
            });
        }

        // Process the timeout event
        self.process_input(myself, state, ConsensusInput::TimeoutElapsed(timeout))
            .await?;

        Ok(())
    }

    async fn wal_reset(&self, height: Ctx::Height) -> Result<(), ActorProcessingErr> {
        let result = ractor::call!(self.wal, WalMsg::Reset, height);

        match result {
            Ok(Ok(())) => {
                // Success
            }
            Ok(Err(e)) => {
                error!(%height, "Failed to reset WAL: {e}");
                return Err(e
                    .wrap_err(format!("Failed to reset WAL for height {height}"))
                    .into());
            }
            Err(e) => {
                error!(%height, "Failed to send Reset command to WAL actor: {e}");
                return Err(eyre!(e)
                    .wrap_err(format!(
                        "Failed to send Reset command to WAL actor for height {height}"
                    ))
                    .into());
            }
        }

        Ok(())
    }

    async fn wal_fetch(
        &self,
        height: Ctx::Height,
    ) -> Result<Vec<io::Result<ConsensusInput<Ctx>>>, ActorProcessingErr> {
        let result = ractor::call!(self.wal, WalMsg::StartedHeight, height)?;

        match result {
            Ok(entries) if entries.is_empty() => {
                debug!(%height, "No WAL entries to replay");

                // Nothing to replay
                Ok(Vec::new())
            }

            Ok(entries) => {
                info!("Found {} WAL entries", entries.len());

                Ok(entries)
            }

            Err(e) => {
                error!(%height, "Error when notifying WAL of started height: {e}");

                self.tx_event.send(|| Event::WalResetError(Arc::new(e)));

                Err(eyre!("Failed to fetch WAL entries for height {height}").into())
            }
        }
    }

    async fn wal_replay(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        height: Ctx::Height,
        entries: Vec<io::Result<ConsensusInput<Ctx>>>,
    ) {
        assert_eq!(state.phase, Phase::Recovering);

        if entries.is_empty() {
            return;
        }

        info!("Replaying {} WAL entries", entries.len());

        state
            .consensus
            .as_mut()
            .expect("StartHeight initializes consensus before WAL replay")
            .record_recovered_own_precommit_extensions(
                entries.iter().filter_map(|entry| entry.as_ref().ok()),
            );

        self.tx_event
            .send(|| Event::WalReplayBegin(height, entries.len()));

        // Index our own recorded messages before feeding anything back in.
        // This way we don't need any assumption on the position of votes/proposals in the WAL
        state
            .consensus
            .as_mut()
            .expect("Consensus not started")
            .index_wal_entries(entries.iter().flatten());

        // Replay WAL entries, stopping at the first corrupted entry
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    let error = Arc::new(e);

                    self.tx_event
                        .send(|| Event::WalCorrupted(Arc::clone(&error)));

                    hang_on_safety_failure(&self.node, async { Err::<(), _>(error) }, |e| {
                        format!("Corrupted WAL entry encountered: {e}")
                    })
                    .await;

                    unreachable!()
                }
            };

            self.tx_event.send(|| Event::WalReplayEntry(entry.clone()));

            info!("Replaying entry: {entry:?}");

            // Replay starts on a clean driver, so the timer-cancel side-effect performed by
            // `timeout_elapsed` for live timeouts is unnecessary here — feeding the input
            // directly is sufficient.
            if let Err(e) = self.process_input(myself, state, entry).await {
                error!("Error when replaying entry: {e}");

                let e = Arc::new(e);
                self.tx_event.send({
                    let e = Arc::clone(&e);
                    || Event::WalReplayError(e)
                });

                hang_on_safety_failure(&self.node, async { Err::<(), _>(e) }, |e| {
                    format!("WAL replay entry cannot be applied: {e}")
                })
                .await;

                unreachable!()
            }
        }

        // Past the end of the log: everything from here on is derived for the first time.
        state
            .consensus
            .as_mut()
            .expect("Consensus not started")
            .reset_entries_index();

        self.tx_event.send(|| Event::WalReplayDone(state.height()));
    }

    fn get_value(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        height: Ctx::Height,
        round: Round,
        timeout: Duration,
    ) -> Result<(), ActorProcessingErr> {
        // Call `GetValue` on the Host actor, and forward the reply
        // to the current actor, wrapping it in `Msg::ProposeValue`.
        self.host.call_and_forward(
            |reply_to| HostMsg::GetValue {
                height,
                round,
                timeout,
                reply_to,
            },
            myself,
            Msg::<Ctx>::ProposeValue,
            None,
        )?;

        Ok(())
    }

    async fn extend_vote(
        &self,
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        vote_extension_policy: VoteExtensionPolicy,
    ) -> Result<Option<Ctx::Extension>, ActorProcessingErr> {
        ractor::call!(self.host, |reply_to| HostMsg::ExtendVote {
            height,
            round,
            value_id,
            vote_extension_policy,
            reply_to
        })
        .map_err(|e| eyre!("Failed to extend vote: {e:?}").into())
    }

    /// Ask the application about every extension in one call.
    ///
    /// The answers must arrive one per extension, in the order the extensions
    /// were given, each naming the address it answers for. `Err` means the
    /// reply cannot be applied.
    async fn verify_vote_extensions(
        &self,
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        extensions: Vec<(Ctx::Address, Ctx::Extension)>,
    ) -> Result<VoteExtensionVerdicts<Ctx>, String> {
        let asked: Vec<Ctx::Address> = extensions
            .iter()
            .map(|(address, _)| address.clone())
            .collect();

        let answers = ractor::call!(self.host, |reply_to| HostMsg::VerifyVoteExtensions {
            height,
            round,
            value_id,
            extensions,
            reply_to
        })
        .map_err(|e| format!("Failed to verify vote extensions: {e:?}"))?;

        let answered: Vec<_> = answers.iter().map(|(address, _)| address.clone()).collect();
        if let Some(reason) = vote_extension_reply_mismatch(&asked, &answered) {
            return Err(reason);
        }

        Ok(answers)
    }

    /// Ask the application about every extension carried by a commit
    /// certificate whose policy and signatures already verified.
    ///
    /// `Ok` is the certificate verdict: the application refused the first
    /// extension it rejected. `Err` is a local reply that cannot be applied;
    /// the caller returns it from the handler so the height is re-requested
    /// and the peer is not faulted.
    async fn verify_certificate_vote_extensions(
        &self,
        certificate: &ExtendedCommitCertificate<Ctx>,
    ) -> Result<Result<(), CertificateError<Ctx>>, String> {
        let extensions: Vec<_> = certificate
            .commit_signatures
            .iter()
            .filter_map(|signature| {
                signature
                    .extension
                    .as_ref()
                    .map(|extension| (signature.address.clone(), extension.message.clone()))
            })
            .collect();

        if extensions.is_empty() {
            return Ok(Ok(()));
        }

        let results = self
            .verify_vote_extensions(
                certificate.height,
                certificate.round,
                certificate.value_id.clone(),
                extensions,
            )
            .await?;

        for (address, result) in results {
            if let Err(e) = result {
                warn!(
                    certificate.height = %certificate.height,
                    certificate.round = %certificate.round,
                    validator = %address,
                    "Synced commit certificate rejected by application vote-extension check: {e}"
                );

                // Signatures were already verified. Either application error
                // is a refusal of the extension contents.
                return Ok(Err(CertificateError::InvalidVoteExtension(address)));
            }
        }

        Ok(Ok(()))
    }

    async fn wal_append(
        &self,
        height: Ctx::Height,
        entry: ConsensusInput<Ctx>,
        phase: Phase,
        is_validator: bool,
    ) -> Result<(), WalFailure> {
        if phase == Phase::Recovering || !is_validator {
            // During recovery we replay rather than write; non-validators don't
            // persist — neither is an error.
            return Ok(());
        }

        match ractor::call!(self.wal, WalMsg::Append, height, entry) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(WalFailure::Write(e)),
            Err(e) => Err(WalFailure::Transport(e.to_string())),
        }
    }

    async fn wal_flush(&self, phase: Phase, is_validator: bool) -> Result<(), WalFailure> {
        if phase == Phase::Recovering || !is_validator {
            return Ok(());
        }

        match ractor::call!(self.wal, WalMsg::Flush) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(WalFailure::Flush(e)),
            Err(e) => Err(WalFailure::Transport(e.to_string())),
        }
    }

    async fn handle_effect(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: HandlerState<'_, Ctx>,
        effect: Effect<Ctx>,
    ) -> Result<Resume<Ctx>, ActorProcessingErr> {
        match effect {
            Effect::CancelAllTimeouts(r) => {
                state.timers.cancel_all();
                Ok(r.resume_with(()))
            }

            Effect::CancelTimeout(timeout, r) => {
                state.timers.cancel(&timeout);
                Ok(r.resume_with(()))
            }

            Effect::ScheduleTimeout(timeout, r) => {
                let duration = state.timeouts.duration_for(timeout);
                state.timers.start_timer(timeout, duration);

                Ok(r.resume_with(()))
            }

            Effect::StartRound(height, round, proposer, role, r) => {
                // Flush prior-round writes before starting a new round.
                // Belt-and-suspenders: the publish-path flush is the primary
                // barrier, but this guards against an effect reordering that
                // would skip it.
                hang_on_safety_failure(
                    &self.node,
                    self.wal_flush(state.phase, state.is_validator),
                    |e| format!("wal_flush before StartRound (h={height}, r={round}) failed: {e}"),
                )
                .await;

                let undecided_values =
                    ractor::call!(self.host, |reply_to| HostMsg::StartedRound {
                        height,
                        round,
                        proposer: proposer.clone(),
                        role,
                        reply_to,
                    })?;

                for value in undecided_values {
                    let _ = myself.cast(Msg::ReceivedProposedValue(value, ValueOrigin::Consensus));
                }

                self.tx_event
                    .send(|| Event::StartedRound(height, round, proposer, role));

                Ok(r.resume_with(()))
            }

            Effect::SignProposal(proposal, r) => {
                let start = Instant::now();

                let signed_proposal = self.signer().sign_proposal(proposal).await?;

                self.metrics
                    .signature_signing_time
                    .observe(start.elapsed().as_secs_f64());

                Ok(r.resume_with(signed_proposal))
            }

            Effect::SignVote(vote, r) => {
                let start = Instant::now();

                let signed_vote = self.signer().sign_vote(vote).await?;

                self.metrics
                    .signature_signing_time
                    .observe(start.elapsed().as_secs_f64());

                Ok(r.resume_with(signed_vote))
            }

            Effect::VerifySignature(msg, pk, r) => {
                use malachitebft_core_consensus::ConsensusMsg as Msg;

                let start = Instant::now();

                let result = match msg.message {
                    Msg::Vote(v) => {
                        self.verifier
                            .verify_signed_vote(&v, &msg.signature, &pk)
                            .await
                    }
                    Msg::Proposal(p) => {
                        self.verifier
                            .verify_signed_proposal(&p, &msg.signature, &pk)
                            .await
                    }
                };

                self.metrics
                    .signature_verification_time
                    .observe(start.elapsed().as_secs_f64());

                // An incomplete check (`Err`) is treated as invalid. Returning
                // the error from `handle_effect` would make `process!` resume
                // with `Continue`, which then fails the consensus actor as
                // `Error::UnexpectedResume` — a gossip peer must not be able
                // to do that.
                let valid = match result {
                    Ok(result) => result.is_valid(),
                    Err(error) => {
                        warn!(
                            %error,
                            "Signature verification could not complete; treating as invalid"
                        );
                        false
                    }
                };

                Ok(r.resume_with(valid))
            }

            Effect::VerifyCommitCertificate(certificate, validator_set, thresholds, r) => {
                let result = self
                    .verifier
                    .verify_commit_certificate(&self.ctx, &certificate, &validator_set, thresholds)
                    .await;

                Ok(r.resume_with(result))
            }

            Effect::VerifyPolkaCertificate(certificate, validator_set, thresholds, r) => {
                let result = self
                    .verifier
                    .verify_polka_certificate(&self.ctx, &certificate, &validator_set, thresholds)
                    .await;

                Ok(r.resume_with(result))
            }

            Effect::VerifyExtendedCommitCertificate(
                certificate,
                validator_set,
                thresholds,
                vote_extension_policy,
                r,
            ) => {
                let result = self
                    .verifier
                    .verify_extended_commit_certificate(
                        &self.ctx,
                        &certificate,
                        &validator_set,
                        thresholds,
                        vote_extension_policy,
                    )
                    .await;

                if result.is_err() {
                    return Ok(r.resume_with(result));
                }

                // Policy and signatures hold. The application has not yet seen
                // the extensions themselves, so ask about all of them at once,
                // as the live precommit path asks about its one.
                let result = match self.verify_certificate_vote_extensions(&certificate).await {
                    Ok(result) => result,
                    Err(reason) => {
                        warn!(
                            certificate.height = %certificate.height,
                            certificate.round = %certificate.round,
                            "Synced certificate vote-extension reply could not be applied; re-requesting the height: {reason}"
                        );
                        return Err(eyre!(reason).into());
                    }
                };

                Ok(r.resume_with(result))
            }

            Effect::VerifyRoundCertificate(certificate, validator_set, thresholds, r) => {
                let result = self
                    .verifier
                    .verify_round_certificate(&self.ctx, &certificate, &validator_set, thresholds)
                    .await;

                Ok(r.resume_with(result))
            }

            Effect::ExtendVote(height, round, value_id, vote_extension_policy, r) => {
                // Only emitted under VoteExtensionPolicy::Required.
                let Some(extension) = self
                    .extend_vote(height, round, value_id.clone(), vote_extension_policy)
                    .await?
                else {
                    error!(%height, %round, %value_id, "Host returned no vote extension");
                    return Ok(r.resume_with(None));
                };

                let scope = VoteExtensionScope::new(
                    height,
                    round,
                    value_id.clone(),
                    self.params.address.clone(),
                );

                match self.signer().sign_vote_extension(scope, extension).await {
                    Ok(signed_extension) => Ok(r.resume_with(Some(signed_extension))),
                    Err(e) => {
                        error!(%height, %round, %value_id, "Failed to sign vote extension: {e}");
                        Ok(r.resume_with(None))
                    }
                }
            }

            Effect::VerifyVoteExtension(
                height,
                round,
                value_id,
                validator_address,
                signed_extension,
                pk,
                r,
            ) => {
                let scope = VoteExtensionScope::new(
                    height,
                    round,
                    value_id.clone(),
                    validator_address.clone(),
                );

                let result = self
                    .verifier
                    .verify_signed_vote_extension(
                        &scope,
                        &signed_extension.message,
                        &signed_extension.signature,
                        &pk,
                    )
                    .await?;

                if result.is_invalid() {
                    return Ok(r.resume_with(Err(VoteExtensionError::InvalidSignature)));
                }

                let extension = signed_extension.message;
                let verdicts = match self
                    .verify_vote_extensions(
                        height,
                        round,
                        value_id.clone(),
                        vec![(validator_address.clone(), extension)],
                    )
                    .await
                {
                    Ok(verdicts) => verdicts,
                    Err(reason) => {
                        warn!(
                            %height,
                            %round,
                            %value_id,
                            validator = %validator_address,
                            "Vote extension reply could not be applied; dropping the vote: {reason}"
                        );
                        return Ok(r.resume_with(Err(VoteExtensionError::InvalidVoteExtension)));
                    }
                };

                Ok(r.resume_with(
                    verdicts
                        .into_iter()
                        .next()
                        .map(|(_, result)| result)
                        .unwrap_or(Err(VoteExtensionError::InvalidVoteExtension)),
                ))
            }

            Effect::PublishConsensusMsg(msg, r) => {
                // Flush the WAL before the signed message escapes to peers.
                // The entry was appended by `WalAppend`; broadcasting ahead of a
                // durable WAL is the double-sign vector documented on `WalFailure`.
                hang_on_safety_failure(
                    &self.node,
                    self.wal_flush(state.phase, state.is_validator),
                    |e| format!("wal_flush before PublishConsensusMsg failed: {e}"),
                )
                .await;

                // Notify any subscribers that we are about to publish a message
                self.tx_event.send(|| Event::Published(msg.clone()));

                self.network
                    .cast(NetworkMsg::PublishConsensusMsg(msg))
                    .map_err(|e| eyre!("Error when broadcasting consensus message: {e:?}"))?;

                Ok(r.resume_with(()))
            }

            Effect::PublishLivenessMsg(msg, r) => {
                match msg {
                    LivenessMsg::Vote(ref msg) => {
                        self.tx_event.send(|| Event::RepublishVote(msg.clone()));
                    }
                    LivenessMsg::PolkaCertificate(ref certificate) => {
                        self.tx_event
                            .send(|| Event::PolkaCertificate(certificate.clone()));
                    }
                    LivenessMsg::SkipRoundCertificate(ref certificate) => {
                        self.tx_event
                            .send(|| Event::SkipRoundCertificate(certificate.clone()));
                    }
                }

                self.network
                    .cast(NetworkMsg::PublishLivenessMsg(msg))
                    .map_err(|e| eyre!("Error when broadcasting liveness message: {e:?}"))?;

                Ok(r.resume_with(()))
            }

            Effect::RepublishVote(msg, r) => {
                // Notify any subscribers that we are about to rebroadcast a vote
                self.tx_event.send(|| Event::RepublishVote(msg.clone()));

                self.network
                    .cast(NetworkMsg::PublishLivenessMsg(LivenessMsg::Vote(msg)))
                    .map_err(|e| eyre!("Error when rebroadcasting vote message: {e:?}"))?;

                Ok(r.resume_with(()))
            }

            Effect::RepublishRoundCertificate(certificate, r) => {
                // Notify any subscribers that we are about to rebroadcast a round certificate
                self.tx_event
                    .send(|| Event::RebroadcastRoundCertificate(certificate.clone()));

                self.network
                    .cast(NetworkMsg::PublishLivenessMsg(
                        LivenessMsg::SkipRoundCertificate(certificate),
                    ))
                    .map_err(|e| {
                        eyre!("Error when rebroadcasting round certificate message: {e:?}")
                    })?;

                Ok(r.resume_with(()))
            }

            Effect::GetValue(height, round, timeout, r) => {
                let timeout_duration = state.timeouts.duration_for(timeout);

                self.get_value(myself, height, round, timeout_duration)
                    .map_err(|e| {
                        eyre!("Error when asking application for value to propose: {e:?}")
                    })?;

                Ok(r.resume_with(()))
            }

            Effect::RestreamProposal(height, round, valid_round, address, value_id, r) => {
                self.host
                    .cast(HostMsg::RestreamValue {
                        height,
                        round,
                        valid_round,
                        address,
                        value_id,
                    })
                    .map_err(|e| eyre!("Error when asking host to restream proposal: {e:?}"))?;

                Ok(r.resume_with(()))
            }

            Effect::Decide(certificate, extensions, r) => {
                assert!(!certificate.commit_signatures.is_empty());

                // Flush the WAL before committing a decision: a decision is
                // terminal for the height, so deciding ahead of a durable WAL
                // could let a restart pick a different value.
                let decide_height = certificate.height;
                hang_on_safety_failure(
                    &self.node,
                    self.wal_flush(state.phase, state.is_validator),
                    move |e| format!("wal_flush before Decide (h={decide_height}) failed: {e}"),
                )
                .await;

                // Notify any subscribers about the decided value
                self.tx_event.send(|| Event::Decided {
                    commit_certificate: certificate.clone(),
                });

                let height = certificate.height;
                let generation = state.commit_generation;

                // Notify the host about the decided value and wait for commit confirmation.
                // When the app replies, the forwarded DecisionCommitted message will notify
                // the sync actor, ensuring the decision is committed before we advertise it.
                self.host
                    .call_and_forward(
                        |reply_to| HostMsg::Decided {
                            certificate,
                            extensions,
                            reply_to,
                        },
                        myself,
                        move |()| Msg::<Ctx>::DecisionCommitted(height, generation),
                        None,
                    )
                    .map_err(|e| eyre!("Error when sending decided value to host: {e:?}"))?;

                Ok(r.resume_with(()))
            }

            Effect::Finalize(certificate, extensions, evidence, r) => {
                assert!(!certificate.commit_signatures.is_empty());

                // Update metrics for equivocation evidence
                let proposal_evidence_count = evidence
                    .proposals
                    .iter()
                    .map(|(_, proposals)| proposals.len())
                    .sum::<usize>();
                let vote_evidence_count = evidence
                    .votes
                    .iter()
                    .map(|(_, votes)| votes.len())
                    .sum::<usize>();
                if proposal_evidence_count > 0 {
                    self.metrics
                        .equivocation_proposals
                        .inc_by(proposal_evidence_count as u64);
                }
                if vote_evidence_count > 0 {
                    self.metrics
                        .equivocation_votes
                        .inc_by(vote_evidence_count as u64);
                }

                if proposal_evidence_count > 0 || vote_evidence_count > 0 {
                    let validator_addresses = evidence
                        .proposals
                        .iter()
                        .map(|(addr, _)| addr.to_string())
                        .chain(evidence.votes.iter().map(|(addr, _)| addr.to_string()))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .join(", ");

                    warn!(
                        height = %certificate.height,
                        round = %certificate.round,
                        proposal_evidence_count,
                        vote_evidence_count,
                        %validator_addresses,
                        "Equivocation evidence observed at finalization"
                    );
                }

                // Notify any subscribers about the finalized value
                self.tx_event.send(|| Event::Finalized {
                    commit_certificate: certificate.clone(),
                    evidence: evidence.clone(),
                });

                info!(
                    height = %certificate.height,
                    round = %certificate.round,
                    total_signatures = certificate.commit_signatures.len(),
                    "Height finalized with commit certificate"
                );

                // Notify the host about the finalized value
                self.host
                    .call_and_forward(
                        |reply_to| HostMsg::Finalized {
                            certificate,
                            extensions,
                            evidence,
                            reply_to,
                        },
                        myself,
                        |next| match next {
                            Next::Start(h, params) => Msg::StartHeight(h, params),
                            Next::Restart(h, params) => Msg::RestartHeight(h, params),
                        },
                        None,
                    )
                    .map_err(|e| eyre!("Error when sending finalized value to host: {e:?}"))?;

                Ok(r.resume_with(()))
            }

            Effect::CertRejectedSyncValue(peer, height, error, r) => {
                if let ConsensusError::InvalidCommitCertificate(certificate, e) = error {
                    error!(
                        %peer,
                        %certificate.height,
                        %certificate.round,
                        "Invalid certificate received: {e}"
                    );

                    match state.sync_request_id {
                        Some(request_id) => {
                            self.sync
                                .send(SyncMsg::PeerFault(peer, certificate.height, request_id))
                        }
                        None => {
                            error!(
                                %peer,
                                %certificate.height,
                                "Rejected value has no originating sync request, re-requesting without attributing the fault"
                            );
                            self.sync.send(SyncMsg::LocalTransientError(height));
                        }
                    }
                } else {
                    self.sync.send(SyncMsg::LocalTransientError(height));
                }

                Ok(r.resume_with(()))
            }

            Effect::CertVerifiedSyncValue(value, proposer, r) => {
                let request_id = state.sync_request_id;
                let certificate_height = value.certificate.height;
                let certificate_round = value.certificate.round;

                let sync = Arc::clone(&self.sync);
                let myself = myself.clone();

                // Attribute the fault to the request that served the value, or fall
                // back to an unattributed re-request if the origin is unknown.
                let notify_peer_fault = {
                    let sync = Arc::clone(&self.sync);
                    let peer = value.peer;

                    move || match request_id {
                        Some(request_id) => {
                            sync.send(SyncMsg::PeerFault(peer, certificate_height, request_id))
                        }
                        None => {
                            error!(
                                %peer,
                                height = %certificate_height,
                                "Synced value has no originating sync request, re-requesting without attributing the fault"
                            );
                            sync.send(SyncMsg::LocalTransientError(certificate_height));
                        }
                    }
                };

                cast_and_handle(
                    &self.host,
                    |reply_to| HostMsg::ProcessSyncedValue {
                        height: certificate_height,
                        round: certificate_round,
                        proposer,
                        value_bytes: value.value_bytes,
                        reply_to,
                    },
                    move |result| match result {
                        Ok(SyncedValueOutcome::Verdict(proposed)) => {
                            if proposed.value.id() == value.certificate.value_id {
                                // Id matches the certificate — forward to consensus.
                                // A locally-invalid validity is still forwarded so the
                                // downstream `maybe_sync_decision` path can surface the
                                // version-skew diagnostic to operators.
                                let _ = myself.cast(Msg::<Ctx>::ReceivedProposedValue(
                                    proposed,
                                    ValueOrigin::Sync,
                                ));
                            } else {
                                // Decoded id disagrees with the certificate the peer
                                // sent: a peer-attributable fault, penalize + re-request.
                                warn!(
                                    peer = %value.peer,
                                    height = %certificate_height,
                                    proposed.value_id = %proposed.value.id(),
                                    certificate.value_id = %value.certificate.value_id,
                                    "Synced value id does not match commit certificate, rejecting"
                                );
                                notify_peer_fault();
                            }
                        }
                        Ok(SyncedValueOutcome::PeerFault) => {
                            // Peer-attributable fault (e.g. undecodable bytes):
                            // penalize the peer and re-request from another.
                            warn!(
                                peer = %value.peer,
                                height = %certificate_height,
                                "Host flagged synced value as a peer-attributable fault"
                            );
                            notify_peer_fault();
                        }
                        // Host-reported transient failure and a dropped reply port
                        // (host death / restart) are the same recovery: free the
                        // pending request slot without blaming the serving peer.
                        Ok(SyncedValueOutcome::LocalTransientError) | Err(ReplyDropped) => {
                            debug!(
                                peer = %value.peer,
                                height = %certificate_height,
                                "Host hit a local/transient error processing synced value; re-requesting without penalizing the peer"
                            );
                            sync.send(SyncMsg::LocalTransientError(certificate_height));
                        }
                    },
                )?;

                Ok(r.resume_with(()))
            }

            Effect::WalAppend(height, entry, r) => {
                // Persist the signed message or timeout before `PublishConsensusMsg`
                // broadcasts it — the primary double-sign vector, see `WalFailure`.
                hang_on_safety_failure(
                    &self.node,
                    self.wal_append(height, entry, state.phase, state.is_validator),
                    move |e| format!("wal_append at height {height} failed: {e}"),
                )
                .await;
                Ok(r.resume_with(()))
            }
        }
    }
}

#[async_trait]
impl<Ctx> Actor for Consensus<Ctx>
where
    Ctx: Context,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = ();

    #[tracing::instrument(
        name = "consensus",
        parent = &self.span,
        skip_all,
    )]
    async fn pre_start(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        _args: (),
    ) -> Result<State<Ctx>, ActorProcessingErr> {
        info!("Consensus is starting");

        self.network
            .cast(NetworkMsg::Subscribe(Box::new(myself.clone())))?;

        Ok(State {
            timers: Timers::new(Box::new(myself)),
            timeouts: Ctx::Timeouts::default(),
            consensus: None,
            connected_peers: BTreeSet::new(),
            phase: Phase::Unstarted,
            is_validator: false,
            msg_buffer: MessageBuffer::new(MAX_BUFFER_SIZE),
            commit_generation: CommitGeneration::default(),
        })
    }

    #[tracing::instrument(
        name = "consensus",
        parent = &self.span,
        skip_all,
        fields(height = %state.height(), round = %state.round())
    )]
    async fn post_start(
        &self,
        _myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        info!("Consensus has started");

        state.timers.cancel_all();
        Ok(())
    }

    #[tracing::instrument(
        name = "consensus",
        parent = &self.span,
        skip_all,
        fields(
            height = %span_height(state.height(), &msg),
            round = %span_round(state.round(), &msg)
        )
    )]
    async fn handle(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        if state.phase != Phase::Running && should_buffer(&msg) {
            let _span = error_span!("buffer", phase = ?state.phase).entered();

            // Sync messages only exist once consensus is running: the sync actor
            // issues no request before `SyncMsg::StartedHeight`.
            debug_assert!(
                !is_sync_application_msg(&msg),
                "sync message reached the buffer"
            );

            if let Err(BufferFull(msg)) = state.msg_buffer.buffer(msg) {
                warn!("Buffer is full, dropping message: {msg}");
                self.report_dropped_msg(&msg, DropReason::BufferFull);
            }

            return Ok(());
        }

        self.handle_msg(myself, state, msg).await
    }

    #[tracing::instrument(
        name = "consensus",
        parent = &self.span,
        skip_all,
        fields(
            height = %state.height(),
            round = %state.round()
        )
    )]
    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        info!("Consensus has stopped");
        state.timers.cancel_all();
        Ok(())
    }
}

/// Whether `msg` has to wait in the buffer until consensus reaches `Phase::Running`.
///
/// Matches exhaustively so that a new [`Msg`] or [`NetworkEvent`] variant has to be
/// classified here rather than silently defaulting to being buffered.
fn should_buffer<Ctx: Context>(msg: &Msg<Ctx>) -> bool {
    match msg {
        // Drive the actor into `Running`, or are answered from state that exists
        // in every phase.
        Msg::StartHeight(..)
        | Msg::RestartHeight(..)
        | Msg::DecisionCommitted(..)
        | Msg::DumpState(..) => false,

        // Inputs to the consensus state machine: only meaningful once a height runs.
        Msg::TimeoutElapsed(..)
        | Msg::ProposeValue(..)
        | Msg::ReceivedProposedValue(..)
        | Msg::ProcessSyncResponse(..) => true,

        Msg::NetworkEvent(event) => match event {
            // Peer bookkeeping and sync plumbing, independent of the current height.
            NetworkEvent::Listening(..)
            | NetworkEvent::PeerConnected(..)
            | NetworkEvent::PeerDisconnected(..)
            | NetworkEvent::PeerSubscribed(..)
            | NetworkEvent::ValidatorProofReceived { .. }
            | NetworkEvent::Status(..)
            | NetworkEvent::SyncRequest(..)
            | NetworkEvent::SyncResponse(..)
            | NetworkEvent::SyncRequestFailed(..)
            | NetworkEvent::SyncInboundRequestFailed(..) => false,

            // Consensus messages from peers.
            NetworkEvent::Vote(..)
            | NetworkEvent::Proposal(..)
            | NetworkEvent::ProposalPart(..)
            | NetworkEvent::PolkaCertificate(..)
            | NetworkEvent::RoundCertificate(..) => true,
        },
    }
}

/// `None` when each answer names the address that was asked, in order.
fn vote_extension_reply_mismatch<A>(asked: &[A], answered: &[A]) -> Option<String>
where
    A: PartialEq + fmt::Display,
{
    if answered.len() != asked.len() {
        return Some(format!(
            "Host answered {} of {} vote extensions",
            answered.len(),
            asked.len()
        ));
    }

    for (got, expected) in answered.iter().zip(asked) {
        if got != expected {
            return Some(format!(
                "Host answered for validator {got} where {expected} was asked"
            ));
        }
    }

    None
}

/// Whether `msg` carries a value obtained through the sync protocol.
///
/// Matches exhaustively so that a new [`Msg`] variant has to be classified here
/// rather than defaulting to the loss-tolerant side.
fn is_sync_application_msg<Ctx: Context>(msg: &Msg<Ctx>) -> bool {
    match msg {
        Msg::ProcessSyncResponse(..) | Msg::ReceivedProposedValue(_, ValueOrigin::Sync) => true,
        Msg::ReceivedProposedValue(_, ValueOrigin::Consensus)
        | Msg::StartHeight(..)
        | Msg::RestartHeight(..)
        | Msg::NetworkEvent(..)
        | Msg::TimeoutElapsed(..)
        | Msg::ProposeValue(..)
        | Msg::DecisionCommitted(..)
        | Msg::DumpState(..) => false,
    }
}

/// Use the height we are about to start instead of the consensus state height
/// for the tracing span of the Consensus actor when starting a new height.
fn span_height<Ctx: Context>(height: Ctx::Height, msg: &Msg<Ctx>) -> Ctx::Height {
    match msg {
        Msg::StartHeight(h, _) | Msg::RestartHeight(h, _) => *h,
        _ => height,
    }
}

/// Use round 0 instead of the consensus state round for the tracing span of
/// the Consensus actor when starting a new height.
fn span_round<Ctx: Context>(round: Round, msg: &Msg<Ctx>) -> Round {
    match msg {
        Msg::StartHeight(_, _) | Msg::RestartHeight(_, _) => Round::new(0),
        _ => round,
    }
}

#[cfg(test)]
mod tests {
    use super::vote_extension_reply_mismatch;

    #[test]
    fn vote_extension_reply_accepts_matching_addresses_in_order() {
        let asked = ["a", "b"];
        assert!(vote_extension_reply_mismatch(&asked, &["a", "b"]).is_none());
    }

    #[test]
    fn vote_extension_reply_rejects_a_short_answer() {
        let asked = ["a", "b"];
        let reason = vote_extension_reply_mismatch(&asked, &["a"]).expect("short reply");
        assert!(reason.contains("1 of 2"), "{reason}");
    }

    #[test]
    fn vote_extension_reply_rejects_a_long_answer() {
        let asked = ["a"];
        let reason = vote_extension_reply_mismatch(&asked, &["a", "b"]).expect("long reply");
        assert!(reason.contains("2 of 1"), "{reason}");
    }

    #[test]
    fn vote_extension_reply_rejects_permuted_addresses() {
        let asked = ["a", "b"];
        let reason = vote_extension_reply_mismatch(&asked, &["b", "a"]).expect("permuted reply");
        assert_eq!(reason, "Host answered for validator b where a was asked");
    }
}
