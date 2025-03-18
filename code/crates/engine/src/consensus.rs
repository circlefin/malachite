use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use async_recursion::async_recursion;
use async_trait::async_trait;
use derive_where::derive_where;
use eyre::eyre;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::time::Instant;
use tracing::{debug, error, error_span, info, warn};

use malachitebft_codec as codec;
use malachitebft_config::TimeoutConfig;
use malachitebft_core_consensus::{
    Effect, PeerId, Resumable, Resume, SignedConsensusMsg, VoteExtensionError, VoteSyncMode,
};
use malachitebft_core_types::{
    Context, Round, SigningProvider, SigningProviderExt, Timeout, TimeoutKind, ValidatorSet,
    ValueId, ValueOrigin,
};
use malachitebft_metrics::Metrics;
use malachitebft_sync::{
    self as sync, InboundRequestId, Response, ValueResponse, VoteSetRequest, VoteSetResponse,
};

use crate::host::{HostMsg, HostRef, LocallyProposedValue, ProposedValue};
use crate::network::{NetworkEvent, NetworkMsg, NetworkRef};
use crate::sync::Msg as SyncMsg;
use crate::sync::SyncRef;
use crate::util::events::{Event, TxEvent};
use crate::util::msg_buffer::MessageBuffer;
use crate::util::streaming::StreamMessage;
use crate::util::timers::{TimeoutElapsed, TimerScheduler};
use crate::wal::{Msg as WalMsg, WalEntry, WalRef};

pub use malachitebft_core_consensus::Error as ConsensusError;
pub use malachitebft_core_consensus::Params as ConsensusParams;
pub use malachitebft_core_consensus::State as ConsensusState;

/// Codec for consensus messages.
///
/// This trait is automatically implemented for any type that implements:
/// - [`codec::Codec<Ctx::ProposalPart>`]
/// - [`codec::Codec<SignedConsensusMsg<Ctx>>`]
/// - [`codec::Codec<StreamMessage<Ctx::ProposalPart>>`]
pub trait ConsensusCodec<Ctx>
where
    Ctx: Context,
    Self: codec::Codec<Ctx::ProposalPart>,
    Self: codec::Codec<SignedConsensusMsg<Ctx>>,
    Self: codec::Codec<StreamMessage<Ctx::ProposalPart>>,
{
}

impl<Ctx, Codec> ConsensusCodec<Ctx> for Codec
where
    Ctx: Context,
    Self: codec::Codec<Ctx::ProposalPart>,
    Self: codec::Codec<SignedConsensusMsg<Ctx>>,
    Self: codec::Codec<StreamMessage<Ctx::ProposalPart>>,
{
}

pub type ConsensusRef<Ctx> = ActorRef<Msg<Ctx>>;

pub struct Consensus<Ctx>
where
    Ctx: Context,
{
    ctx: Ctx,
    params: ConsensusParams<Ctx>,
    timeout_config: TimeoutConfig,
    signing_provider: Box<dyn SigningProvider<Ctx>>,
    network: NetworkRef<Ctx>,
    host: HostRef<Ctx>,
    wal: WalRef<Ctx>,
    sync: Option<SyncRef<Ctx>>,
    metrics: Metrics,
    tx_event: TxEvent<Ctx>,
    span: tracing::Span,
}

pub type ConsensusMsg<Ctx> = Msg<Ctx>;

#[derive_where(Debug)]
pub enum Msg<Ctx: Context> {
    /// Start consensus for the given height with the given validator set
    StartHeight(Ctx::Height, Ctx::ValidatorSet),

    /// Received an event from the gossip layer
    NetworkEvent(NetworkEvent<Ctx>),

    /// A timeout has elapsed
    TimeoutElapsed(TimeoutElapsed<Timeout>),

    /// The proposal builder has built a value and can be used in a new proposal consensus message
    ProposeValue(Ctx::Height, Round, Ctx::Value),

    /// Received and assembled the full value proposed by a validator
    ReceivedProposedValue(ProposedValue<Ctx>, ValueOrigin),
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

struct Timeouts {
    config: TimeoutConfig,
}

impl Timeouts {
    pub fn new(config: TimeoutConfig) -> Self {
        Self { config }
    }

    fn reset(&mut self, config: TimeoutConfig) {
        self.config = config;
    }

    fn duration_for(&self, step: TimeoutKind) -> Duration {
        match step {
            TimeoutKind::Propose => self.config.timeout_propose,
            TimeoutKind::Prevote => self.config.timeout_prevote,
            TimeoutKind::Precommit => self.config.timeout_precommit,
            TimeoutKind::Commit => self.config.timeout_commit,
            TimeoutKind::PrevoteTimeLimit => self.config.timeout_step,
            TimeoutKind::PrecommitTimeLimit => self.config.timeout_step,
            TimeoutKind::PrevoteRebroadcast => self.config.timeout_prevote,
            TimeoutKind::PrecommitRebroadcast => self.config.timeout_precommit,
        }
    }

    fn increase_timeout(&mut self, step: TimeoutKind) {
        let c = &mut self.config;
        match step {
            TimeoutKind::Propose => c.timeout_propose += c.timeout_propose_delta,
            TimeoutKind::Prevote => c.timeout_prevote += c.timeout_prevote_delta,
            TimeoutKind::Precommit => c.timeout_precommit += c.timeout_precommit_delta,
            TimeoutKind::Commit => (),
            TimeoutKind::PrevoteTimeLimit => (),
            TimeoutKind::PrecommitTimeLimit => (),
            TimeoutKind::PrevoteRebroadcast => (),
            TimeoutKind::PrecommitRebroadcast => (),
        };
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Phase {
    Unstarted,
    Ready,
    Running,
    Recovering,
}

/// Maximum number of messages to buffer while consensus is
/// in the `Unstarted` or `Recovering` phase
const MAX_BUFFER_SIZE: usize = 1024;

pub struct State<Ctx: Context> {
    /// Scheduler for timers
    timers: Timers,

    /// Timeouts configuration
    timeouts: Timeouts,

    /// The state of the consensus state machine
    consensus: ConsensusState<Ctx>,

    /// The set of peers we are connected to.
    connected_peers: BTreeSet<PeerId>,

    /// The current phase
    phase: Phase,

    /// A buffer of messages that were received while
    /// consensus was `Unstarted` or in the `Recovering` phase
    msg_buffer: MessageBuffer<Ctx>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn height(&self) -> Ctx::Height {
        self.consensus.height()
    }
}

impl<Ctx> Consensus<Ctx>
where
    Ctx: Context,
{
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        ctx: Ctx,
        params: ConsensusParams<Ctx>,
        timeout_config: TimeoutConfig,
        signing_provider: Box<dyn SigningProvider<Ctx>>,
        network: NetworkRef<Ctx>,
        host: HostRef<Ctx>,
        wal: WalRef<Ctx>,
        sync: Option<SyncRef<Ctx>>,
        metrics: Metrics,
        tx_event: TxEvent<Ctx>,
        span: tracing::Span,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let node = Self {
            ctx,
            params,
            timeout_config,
            signing_provider,
            network,
            host,
            wal,
            sync,
            metrics,
            tx_event,
            span,
        };

        let (actor_ref, _) = Actor::spawn(None, node, ()).await?;
        Ok(actor_ref)
    }

    async fn process_input(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        input: ConsensusInput<Ctx>,
    ) -> Result<(), ConsensusError<Ctx>> {
        let height = state.height();

        malachitebft_core_consensus::process!(
            input: input,
            state: &mut state.consensus,
            metrics: &self.metrics,
            with: effect => {
                self.handle_effect(
                    myself,
                    height,
                    &mut state.timers,
                    &mut state.timeouts,
                    state.phase,
                    effect
                ).await
            }
        )
    }

    #[async_recursion]
    async fn process_buffered_msgs(&self, myself: &ActorRef<Msg<Ctx>>, state: &mut State<Ctx>) {
        if state.msg_buffer.is_empty() {
            return;
        }

        info!(count = %state.msg_buffer.len(), "Replaying buffered messages");

        while let Some(msg) = state.msg_buffer.pop() {
            debug!("Replaying buffered message: {msg:?}");

            if let Err(e) = self.handle_msg(myself.clone(), state, msg).await {
                error!("Error when handling buffered message: {e:?}");
            }
        }
    }

    async fn handle_msg(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        msg: Msg<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::StartHeight(height, validator_set) => {
                let result = self
                    .process_input(
                        &myself,
                        state,
                        ConsensusInput::StartHeight(height, validator_set),
                    )
                    .await;

                if let Err(e) = result {
                    error!(%height, "Error when starting height: {e}");
                }

                // Notify the sync actor that we have started a new height
                if let Some(sync) = &self.sync {
                    if let Err(e) = sync.cast(SyncMsg::StartedHeight(height)) {
                        error!(%height, "Error when notifying sync of started height: {e}")
                    }
                }

                self.tx_event.send(|| Event::StartedHeight(height));

                if let Err(e) = self.check_and_replay_wal(&myself, state, height).await {
                    error!(%height, "Error when checking and replaying WAL: {e}");
                }

                self.process_buffered_msgs(&myself, state).await;

                state.phase = Phase::Running;

                Ok(())
            }

            Msg::ProposeValue(height, round, value) => {
                let value_to_propose = LocallyProposedValue {
                    height,
                    round,
                    value: value.clone(),
                };

                let result = self
                    .process_input(
                        &myself,
                        state,
                        ConsensusInput::Propose(value_to_propose.clone()),
                    )
                    .await;

                if let Err(e) = result {
                    error!(%height, %round, "Error when processing ProposeValue message: {e}");
                }

                self.tx_event
                    .send(|| Event::ProposedValue(value_to_propose));

                Ok(())
            }

            Msg::NetworkEvent(event) => {
                match event {
                    NetworkEvent::Listening(address) => {
                        info!(%address, "Listening");

                        if state.phase == Phase::Unstarted {
                            state.phase = Phase::Ready;

                            self.host.cast(HostMsg::ConsensusReady(myself.clone()))?;
                        }
                    }

                    NetworkEvent::PeerConnected(peer_id) => {
                        if !state.connected_peers.insert(peer_id) {
                            // We already saw that peer, ignoring...
                            return Ok(());
                        }

                        info!(%peer_id, "Connected to peer");

                        let validator_set = state.consensus.validator_set();
                        let connected_peers = state.connected_peers.len();
                        let total_peers = validator_set.count() - 1;

                        debug!(connected = %connected_peers, total = %total_peers, "Connected to another peer");

                        self.metrics.connected_peers.inc();
                    }

                    NetworkEvent::PeerDisconnected(peer_id) => {
                        info!(%peer_id, "Disconnected from peer");

                        if state.connected_peers.remove(&peer_id) {
                            self.metrics.connected_peers.dec();
                        }
                    }

                    NetworkEvent::Response(
                        request_id,
                        peer,
                        sync::Response::ValueResponse(ValueResponse { height, value }),
                    ) => {
                        debug!(%height, %request_id, "Received sync response");

                        let Some(value) = value else {
                            error!(%height, %request_id, "Received empty value sync response");
                            return Ok(());
                        };

                        self.host.call_and_forward(
                            |reply_to| HostMsg::ProcessSyncedValue {
                                height: value.certificate.height,
                                round: value.certificate.round,
                                validator_address: state.consensus.address().clone(),
                                value_bytes: value.value_bytes.clone(),
                                reply_to,
                            },
                            &myself,
                            |proposed| {
                                Msg::<Ctx>::ReceivedProposedValue(proposed, ValueOrigin::Sync)
                            },
                            None,
                        )?;

                        if let Err(e) = self
                            .process_input(
                                &myself,
                                state,
                                ConsensusInput::CommitCertificate(value.certificate),
                            )
                            .await
                        {
                            error!(%height, %request_id, "Error when processing received synced block: {e}");

                            let Some(sync) = self.sync.as_ref() else {
                                warn!("Received sync response but sync actor is not available");
                                return Ok(());
                            };

                            if let ConsensusError::InvalidCertificate(certificate, e) = e {
                                sync.cast(SyncMsg::InvalidCertificate(peer, certificate, e))
                                    .map_err(|e| {
                                        eyre!(
                                            "Error when notifying sync of invalid certificate: {e}"
                                        )
                                    })?;
                            }
                        }
                    }

                    NetworkEvent::Request(
                        request_id,
                        peer,
                        sync::Request::VoteSetRequest(VoteSetRequest { height, round }),
                    ) => {
                        debug!(%height, %round, %request_id, %peer, "Received vote set request");

                        if let Err(e) = self
                            .process_input(
                                &myself,
                                state,
                                ConsensusInput::VoteSetRequest(
                                    request_id.to_string(),
                                    height,
                                    round,
                                ),
                            )
                            .await
                        {
                            error!(%peer, %height, %round, "Error when processing VoteSetRequest: {e:?}");
                        }
                    }

                    NetworkEvent::Response(
                        request_id,
                        peer,
                        sync::Response::VoteSetResponse(VoteSetResponse {
                            height,
                            round,
                            vote_set,
                        }),
                    ) => {
                        if vote_set.votes.is_empty() {
                            debug!(%height, %round, %request_id, %peer, "Received an empty vote set response");
                            return Ok(());
                        };

                        debug!(%height, %round, %request_id, %peer, "Received a non-empty vote set response");

                        if let Err(e) = self
                            .process_input(
                                &myself,
                                state,
                                ConsensusInput::VoteSetResponse(vote_set),
                            )
                            .await
                        {
                            error!(%height, %round, %request_id, %peer, "Error when processing VoteSetResponse: {e:?}");
                        }
                    }

                    NetworkEvent::Vote(from, vote) => {
                        if let Err(e) = self
                            .process_input(&myself, state, ConsensusInput::Vote(vote))
                            .await
                        {
                            error!(%from, "Error when processing vote: {e}");
                        }
                    }

                    NetworkEvent::Proposal(from, proposal) => {
                        if state.consensus.params.value_payload.parts_only() {
                            error!(%from, "Properly configured peer should never send proposal messages in BlockPart mode");
                            return Ok(());
                        }

                        if let Err(e) = self
                            .process_input(&myself, state, ConsensusInput::Proposal(proposal))
                            .await
                        {
                            error!(%from, "Error when processing proposal: {e}");
                        }
                    }

                    NetworkEvent::ProposalPart(from, part) => {
                        if state.consensus.params.value_payload.proposal_only() {
                            error!(%from, "Properly configured peer should never send proposal part messages in Proposal mode");
                            return Ok(());
                        }

                        self.host
                            .call_and_forward(
                                |reply_to| HostMsg::ReceivedProposalPart {
                                    from,
                                    part,
                                    reply_to,
                                },
                                &myself,
                                |value| Msg::ReceivedProposedValue(value, ValueOrigin::Consensus),
                                None,
                            )
                            .map_err(|e| {
                                eyre!("Error when forwarding proposal parts to host: {e}")
                            })?;
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

                state.timeouts.increase_timeout(timeout.kind);

                if matches!(
                    timeout.kind,
                    TimeoutKind::Prevote
                        | TimeoutKind::Precommit
                        | TimeoutKind::PrevoteTimeLimit
                        | TimeoutKind::PrecommitTimeLimit
                ) {
                    warn!(step = ?timeout.kind, "Timeout elapsed");

                    state.consensus.print_state();
                }

                let result = self
                    .process_input(&myself, state, ConsensusInput::TimeoutElapsed(timeout))
                    .await;

                if let Err(e) = result {
                    error!("Error when processing TimeoutElapsed message: {e:?}");
                }

                Ok(())
            }

            Msg::ReceivedProposedValue(value, origin) => {
                self.tx_event
                    .send(|| Event::ReceivedProposedValue(value.clone(), origin));

                let result = self
                    .process_input(&myself, state, ConsensusInput::ProposedValue(value, origin))
                    .await;

                if let Err(e) = result {
                    error!("Error when processing ReceivedProposedValue message: {e}");
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
    ) -> Result<(), ActorProcessingErr> {
        // Make sure the associated timer is cancelled
        state.timers.cancel(&timeout);

        // Increase the timeout for the next round
        state.timeouts.increase_timeout(timeout.kind);

        // Print debug information if the timeout is for a prevote or precommit
        if matches!(timeout.kind, TimeoutKind::Prevote | TimeoutKind::Precommit) {
            warn!(step = ?timeout.kind, "Timeout elapsed");
            state.consensus.print_state();
        }

        // Process the timeout event
        self.process_input(myself, state, ConsensusInput::TimeoutElapsed(timeout))
            .await?;

        Ok(())
    }

    async fn check_and_replay_wal(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        height: Ctx::Height,
    ) -> Result<(), ActorProcessingErr> {
        let result = ractor::call!(self.wal, WalMsg::StartedHeight, height)?;

        match result {
            Ok(None) => {
                // Nothing to replay
                debug!(%height, "No WAL entries to replay");
            }
            Ok(Some(entries)) => {
                info!("Found {} WAL entries to replay", entries.len());

                state.phase = Phase::Recovering;

                if let Err(e) = self.replay_wal_entries(myself, state, entries).await {
                    error!(%height, "Failed to replay WAL entries: {e}");
                    self.tx_event.send(|| Event::WalReplayError(Arc::new(e)));
                }
            }
            Err(e) => {
                error!(%height, "Error when notifying WAL of started height: {e}");
                self.tx_event
                    .send(|| Event::WalReplayError(Arc::new(e.into())));
            }
        }

        Ok(())
    }

    async fn replay_wal_entries(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        entries: Vec<WalEntry<Ctx>>,
    ) -> Result<(), ActorProcessingErr> {
        use SignedConsensusMsg::*;

        debug_assert!(!entries.is_empty());

        self.tx_event
            .send(|| Event::WalReplayBegin(state.height(), entries.len()));

        for entry in entries {
            match entry {
                WalEntry::ConsensusMsg(Vote(vote)) => {
                    self.tx_event
                        .send(|| Event::WalReplayConsensus(Vote(vote.clone())));

                    if let Err(e) = self
                        .process_input(myself, state, ConsensusInput::Vote(vote))
                        .await
                    {
                        error!("Error when replaying Vote: {e}");
                    }
                }

                WalEntry::ConsensusMsg(Proposal(proposal)) => {
                    self.tx_event
                        .send(|| Event::WalReplayConsensus(Proposal(proposal.clone())));

                    if let Err(e) = self
                        .process_input(myself, state, ConsensusInput::Proposal(proposal))
                        .await
                    {
                        error!("Error when replaying Proposal: {e}");
                    }
                }

                WalEntry::Timeout(timeout) => {
                    self.tx_event.send(|| Event::WalReplayTimeout(timeout));

                    if let Err(e) = self.timeout_elapsed(myself, state, timeout).await {
                        error!("Error when replaying TimeoutElapsed: {e}");
                    }
                }
            }
        }

        self.tx_event.send(|| Event::WalReplayDone(state.height()));

        Ok(())
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
            |proposed: LocallyProposedValue<Ctx>| {
                Msg::<Ctx>::ProposeValue(proposed.height, proposed.round, proposed.value)
            },
            None,
        )?;

        Ok(())
    }

    async fn get_validator_set(
        &self,
        height: Ctx::Height,
    ) -> Result<Ctx::ValidatorSet, ActorProcessingErr> {
        let validator_set = ractor::call!(self.host, |reply_to| HostMsg::GetValidatorSet {
            height,
            reply_to
        })
        .map_err(|e| eyre!("Failed to get validator set at height {height}: {e:?}"))?;

        Ok(validator_set)
    }

    async fn extend_vote(
        &self,
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
    ) -> Result<Option<Ctx::Extension>, ActorProcessingErr> {
        ractor::call!(self.host, |reply_to| HostMsg::ExtendVote {
            height,
            round,
            value_id,
            reply_to
        })
        .map_err(|e| eyre!("Failed to get earliest block height: {e:?}").into())
    }

    async fn verify_vote_extension(
        &self,
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        extension: Ctx::Extension,
    ) -> Result<Result<(), VoteExtensionError>, ActorProcessingErr> {
        ractor::call!(self.host, |reply_to| HostMsg::VerifyVoteExtension {
            height,
            round,
            value_id,
            extension,
            reply_to
        })
        .map_err(|e| eyre!("Failed to verify vote extension: {e:?}").into())
    }

    async fn wal_append(
        &self,
        height: Ctx::Height,
        entry: WalEntry<Ctx>,
        phase: Phase,
    ) -> Result<(), ActorProcessingErr> {
        if phase == Phase::Recovering {
            return Ok(());
        }

        let result = ractor::call!(self.wal, WalMsg::Append, height, entry);

        match result {
            Ok(Ok(())) => {
                // Success
            }
            Ok(Err(e)) => {
                error!("Failed to append entry to WAL: {e}");
            }
            Err(e) => {
                error!("Failed to send Append command to WAL actor: {e}");
            }
        }

        Ok(())
    }

    async fn wal_flush(&self, phase: Phase) -> Result<(), ActorProcessingErr> {
        if phase == Phase::Recovering {
            return Ok(());
        }

        let result = ractor::call!(self.wal, WalMsg::Flush);

        match result {
            Ok(Ok(())) => {
                // Success
            }
            Ok(Err(e)) => {
                error!("Failed to flush WAL to disk: {e}");
            }
            Err(e) => {
                error!("Failed to send Flush command to WAL: {e}");
            }
        }

        Ok(())
    }

    async fn handle_effect(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        height: Ctx::Height,
        timers: &mut Timers,
        timeouts: &mut Timeouts,
        phase: Phase,
        effect: Effect<Ctx>,
    ) -> Result<Resume<Ctx>, ActorProcessingErr> {
        match effect {
            Effect::ResetTimeouts(r) => {
                timeouts.reset(self.timeout_config);
                Ok(r.resume_with(()))
            }

            Effect::CancelAllTimeouts(r) => {
                timers.cancel_all();
                Ok(r.resume_with(()))
            }

            Effect::CancelTimeout(timeout, r) => {
                timers.cancel(&timeout);
                Ok(r.resume_with(()))
            }

            Effect::ScheduleTimeout(timeout, r) => {
                let duration = timeouts.duration_for(timeout.kind);
                timers.start_timer(timeout, duration);

                Ok(r.resume_with(()))
            }

            Effect::StartRound(height, round, proposer, r) => {
                self.wal_flush(phase).await?;

                self.host.cast(HostMsg::StartedRound {
                    height,
                    round,
                    proposer,
                })?;

                self.tx_event.send(|| Event::StartedRound(height, round));

                Ok(r.resume_with(()))
            }

            Effect::SignProposal(proposal, r) => {
                let start = Instant::now();

                let signed_proposal = self.signing_provider.sign_proposal(proposal);

                self.metrics
                    .signature_signing_time
                    .observe(start.elapsed().as_secs_f64());

                Ok(r.resume_with(signed_proposal))
            }

            Effect::SignVote(vote, r) => {
                let start = Instant::now();

                let signed_vote = self.signing_provider.sign_vote(vote);

                self.metrics
                    .signature_signing_time
                    .observe(start.elapsed().as_secs_f64());

                Ok(r.resume_with(signed_vote))
            }

            Effect::VerifySignature(msg, pk, r) => {
                use malachitebft_core_consensus::ConsensusMsg as Msg;

                let start = Instant::now();

                let valid = match msg.message {
                    Msg::Vote(v) => {
                        self.signing_provider
                            .verify_signed_vote(&v, &msg.signature, &pk)
                    }
                    Msg::Proposal(p) => {
                        self.signing_provider
                            .verify_signed_proposal(&p, &msg.signature, &pk)
                    }
                };

                self.metrics
                    .signature_verification_time
                    .observe(start.elapsed().as_secs_f64());

                Ok(r.resume_with(valid))
            }

            Effect::VerifyCertificate(certificate, validator_set, thresholds, r) => {
                let valid = self.signing_provider.verify_certificate(
                    &certificate,
                    &validator_set,
                    thresholds,
                );

                Ok(r.resume_with(valid))
            }

            Effect::ExtendVote(height, round, value_id, r) => {
                if let Some(extension) = self.extend_vote(height, round, value_id).await? {
                    let signed_extension = self.signing_provider.sign_vote_extension(extension);
                    Ok(r.resume_with(Some(signed_extension)))
                } else {
                    Ok(r.resume_with(None))
                }
            }

            Effect::VerifyVoteExtension(height, round, value_id, signed_extension, pk, r) => {
                let valid = self.signing_provider.verify_signed_vote_extension(
                    &signed_extension.message,
                    &signed_extension.signature,
                    &pk,
                );

                if !valid {
                    return Ok(r.resume_with(Err(VoteExtensionError::InvalidSignature)));
                }

                let result = self
                    .verify_vote_extension(height, round, value_id, signed_extension.message)
                    .await?;

                Ok(r.resume_with(result))
            }

            Effect::Publish(msg, r) => {
                // Sync the WAL to disk before we broadcast the message
                // NOTE: The message has already been append to the WAL by the `WalAppendMessage` effect.
                self.wal_flush(phase).await?;

                // Notify any subscribers that we are about to publish a message
                self.tx_event.send(|| Event::Published(msg.clone()));

                self.network
                    .cast(NetworkMsg::Publish(msg))
                    .map_err(|e| eyre!("Error when broadcasting gossip message: {e:?}"))?;

                Ok(r.resume_with(()))
            }

            Effect::Rebroadcast(msg, r) => {
                // Rebroadcast last vote only if vote sync mode is set to "rebroadcast",
                // otherwise vote set requests are issued automatically by the sync protocol.
                if self.params.vote_sync_mode == VoteSyncMode::Rebroadcast {
                    // Notify any subscribers that we are about to rebroadcast a message
                    self.tx_event.send(|| Event::Rebroadcast(msg.clone()));

                    self.network
                        .cast(NetworkMsg::Publish(SignedConsensusMsg::Vote(msg)))
                        .map_err(|e| eyre!("Error when rebroadcasting vote message: {e:?}"))?;
                }

                Ok(r.resume_with(()))
            }

            Effect::GetValue(height, round, timeout, r) => {
                let timeout_duration = timeouts.duration_for(timeout.kind);

                self.get_value(myself, height, round, timeout_duration)
                    .map_err(|e| eyre!("Error when asking for value to be built: {e:?}"))?;

                Ok(r.resume_with(()))
            }

            Effect::GetValidatorSet(height, r) => {
                let validator_set = self
                    .get_validator_set(height)
                    .await
                    .map_err(|e| warn!("No validator set found for height {height}: {e:?}"))
                    .ok();

                Ok(r.resume_with(validator_set))
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
                    .map_err(|e| eyre!("Error when sending decided value to host: {e:?}"))?;

                Ok(r.resume_with(()))
            }

            Effect::Decide(certificate, extensions, r) => {
                self.wal_flush(phase).await?;

                self.tx_event.send(|| Event::Decided(certificate.clone()));

                let height = certificate.height;

                self.host
                    .cast(HostMsg::Decided {
                        certificate,
                        extensions,
                        consensus: myself.clone(),
                    })
                    .map_err(|e| eyre!("Error when sending decided value to host: {e:?}"))?;

                if let Some(sync) = &self.sync {
                    sync.cast(SyncMsg::Decided(height))
                        .map_err(|e| eyre!("Error when sending decided height to sync: {e:?}"))?;
                }

                Ok(r.resume_with(()))
            }

            Effect::RequestVoteSet(height, round, r) => {
                if let Some(sync) = &self.sync {
                    debug!(%height, %round, "Request sync to obtain the vote set from peers");

                    sync.cast(SyncMsg::RequestVoteSet(height, round))
                        .map_err(|e| eyre!("Error when sending vote set request to sync: {e:?}"))?;

                    self.tx_event
                        .send(|| Event::RequestedVoteSet(height, round));
                }

                Ok(r.resume_with(()))
            }

            Effect::SendVoteSetResponse(request_id_str, height, round, vote_set, r) => {
                let Some(sync) = self.sync.as_ref() else {
                    warn!("Responding to a vote set request but sync actor is not available");
                    return Ok(r.resume_with(()));
                };

                let vote_count = vote_set.len();
                let response =
                    Response::VoteSetResponse(VoteSetResponse::new(height, round, vote_set));

                let request_id = InboundRequestId::new(request_id_str);

                debug!(
                    %height, %round, %request_id, vote.count = %vote_count,
                    "Sending the vote set response"
                );

                self.network
                    .cast(NetworkMsg::OutgoingResponse(request_id.clone(), response))?;

                sync.cast(SyncMsg::SentVoteSetResponse(request_id, height, round))
                    .map_err(|e| {
                        eyre!("Error when notifying Sync about vote set response: {e:?}")
                    })?;

                self.tx_event
                    .send(|| Event::SentVoteSetResponse(height, round, vote_count));

                Ok(r.resume_with(()))
            }

            Effect::WalAppendMessage(msg, r) => {
                self.wal_append(height, WalEntry::ConsensusMsg(msg), phase)
                    .await?;

                Ok(r.resume_with(()))
            }

            Effect::WalAppendTimeout(timeout, r) => {
                self.wal_append(height, WalEntry::Timeout(timeout), phase)
                    .await?;

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
            timeouts: Timeouts::new(self.timeout_config),
            consensus: ConsensusState::new(self.ctx.clone(), self.params.clone()),
            connected_peers: BTreeSet::new(),
            phase: Phase::Unstarted,
            msg_buffer: MessageBuffer::new(MAX_BUFFER_SIZE),
        })
    }

    #[tracing::instrument(
        name = "consensus",
        parent = &self.span,
        skip_all,
        fields(
            height = %state.consensus.height(),
            round = %state.consensus.round()
        )
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
            height = %span_height(state.consensus.height(), &msg),
            round = %span_round(state.consensus.round(), &msg)
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
            state.msg_buffer.buffer(msg);
            return Ok(());
        }

        if let Err(e) = self.handle_msg(myself.clone(), state, msg).await {
            error!("Error when handling message: {e:?}");
        }

        Ok(())
    }

    #[tracing::instrument(
        name = "consensus",
        parent = &self.span,
        skip_all,
        fields(
            height = %state.consensus.height(),
            round = %state.consensus.round()
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

fn should_buffer<Ctx: Context>(msg: &Msg<Ctx>) -> bool {
    !matches!(
        msg,
        Msg::StartHeight(..)
            | Msg::NetworkEvent(NetworkEvent::Listening(..))
            | Msg::NetworkEvent(NetworkEvent::PeerConnected(..))
            | Msg::NetworkEvent(NetworkEvent::PeerDisconnected(..))
    )
}

/// Use the height we are about to start instead of the consensus state height
/// for the tracing span of the Consensus actor when starting a new height.
fn span_height<Ctx: Context>(height: Ctx::Height, msg: &Msg<Ctx>) -> Ctx::Height {
    if let Msg::StartHeight(h, _) = msg {
        *h
    } else {
        height
    }
}

/// Use round 0 instead of the consensus state round for the tracing span of
/// the Consensus actor when starting a new height.
fn span_round<Ctx: Context>(round: Round, msg: &Msg<Ctx>) -> Round {
    if let Msg::StartHeight(_, _) = msg {
        Round::new(0)
    } else {
        round
    }
}
