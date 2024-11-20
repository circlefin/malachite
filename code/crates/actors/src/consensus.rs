use std::collections::BTreeSet;
use std::time::Duration;

use async_trait::async_trait;
use eyre::eyre;
use libp2p::PeerId;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tracing::{debug, error, info, warn};

use malachite_blocksync as blocksync;
use malachite_common::{
    CommitCertificate, Context, Round, SignedExtension, Timeout, TimeoutStep, ValidatorSet,
};
use malachite_config::TimeoutConfig;
use malachite_consensus::{Effect, Resume};
use malachite_metrics::Metrics;

use crate::block_sync::BlockSyncRef;
use crate::block_sync::Msg as BlockSyncMsg;
use crate::gossip_consensus::{GossipConsensusRef, GossipEvent, Msg as GossipConsensusMsg, Status};
use crate::host::{HostMsg, HostRef, LocallyProposedValue, ProposedValue};
use crate::util::forward::forward;
use crate::util::timers::{TimeoutElapsed, TimerScheduler};

pub use malachite_consensus::Error as ConsensusError;
pub use malachite_consensus::Params as ConsensusParams;
pub use malachite_consensus::State as ConsensusState;

pub type ConsensusRef<Ctx> = ActorRef<Msg<Ctx>>;

pub type TxDecision<Ctx> = broadcast::Sender<CommitCertificate<Ctx>>;

pub struct Consensus<Ctx>
where
    Ctx: Context,
{
    ctx: Ctx,
    params: ConsensusParams<Ctx>,
    timeout_config: TimeoutConfig,
    gossip_consensus: GossipConsensusRef<Ctx>,
    host: HostRef<Ctx>,
    block_sync: Option<BlockSyncRef<Ctx>>,
    metrics: Metrics,
    tx_decision: Option<TxDecision<Ctx>>,
}

pub type ConsensusMsg<Ctx> = Msg<Ctx>;

pub enum Msg<Ctx: Context> {
    /// Start consensus for the given height
    StartHeight(Ctx::Height),

    /// Received an event from the gossip layer
    GossipEvent(GossipEvent<Ctx>),

    /// A timeout has elapsed
    TimeoutElapsed(TimeoutElapsed<Timeout>),

    /// The proposal builder has built a value and can be used in a new proposal consensus message
    ProposeValue(Ctx::Height, Round, Ctx::Value, Option<SignedExtension<Ctx>>),

    /// Received and assembled the full value proposed by a validator
    ReceivedProposedValue(ProposedValue<Ctx>),

    /// Get the status of the consensus state machine
    GetStatus(RpcReplyPort<Status<Ctx>>),
}

type ConsensusInput<Ctx> = malachite_consensus::Input<Ctx>;

impl<Ctx: Context> From<TimeoutElapsed<Timeout>> for Msg<Ctx> {
    fn from(msg: TimeoutElapsed<Timeout>) -> Self {
        Msg::TimeoutElapsed(msg)
    }
}

type Timers<Ctx> = TimerScheduler<Timeout, Msg<Ctx>>;

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

    fn duration_for(&self, step: TimeoutStep) -> Duration {
        match step {
            TimeoutStep::Propose => self.config.timeout_propose,
            TimeoutStep::Prevote => self.config.timeout_prevote,
            TimeoutStep::Precommit => self.config.timeout_precommit,
            TimeoutStep::Commit => self.config.timeout_commit,
        }
    }

    fn increase_timeout(&mut self, step: TimeoutStep) {
        let c = &mut self.config;
        match step {
            TimeoutStep::Propose => c.timeout_propose += c.timeout_propose_delta,
            TimeoutStep::Prevote => c.timeout_prevote += c.timeout_prevote_delta,
            TimeoutStep::Precommit => c.timeout_precommit += c.timeout_precommit_delta,
            TimeoutStep::Commit => (),
        };
    }
}

pub struct State<Ctx: Context> {
    /// Scheduler for timers
    timers: Timers<Ctx>,

    /// Timeouts configuration
    timeouts: Timeouts,

    /// The state of the consensus state machine
    consensus: ConsensusState<Ctx>,

    /// The set of peers we are connected to.
    connected_peers: BTreeSet<PeerId>,
}

impl<Ctx> Consensus<Ctx>
where
    Ctx: Context,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: Ctx,
        params: ConsensusParams<Ctx>,
        timeout_config: TimeoutConfig,
        gossip_consensus: GossipConsensusRef<Ctx>,
        host: HostRef<Ctx>,
        block_sync: Option<BlockSyncRef<Ctx>>,
        metrics: Metrics,
        tx_decision: Option<TxDecision<Ctx>>,
    ) -> Self {
        Self {
            ctx,
            params,
            timeout_config,
            gossip_consensus,
            host,
            block_sync,
            metrics,
            tx_decision,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        ctx: Ctx,
        params: ConsensusParams<Ctx>,
        timeout_config: TimeoutConfig,
        gossip_consensus: GossipConsensusRef<Ctx>,
        host: HostRef<Ctx>,
        block_sync: Option<BlockSyncRef<Ctx>>,
        metrics: Metrics,
        tx_decision: Option<TxDecision<Ctx>>,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let node = Self::new(
            ctx,
            params,
            timeout_config,
            gossip_consensus,
            host,
            block_sync,
            metrics,
            tx_decision,
        );

        let (actor_ref, _) = Actor::spawn(None, node, ()).await?;
        Ok(actor_ref)
    }

    async fn process_input(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        input: ConsensusInput<Ctx>,
    ) -> Result<(), ConsensusError<Ctx>> {
        // Notify the BlockSync actor that we have started a new height
        if let (ConsensusInput::StartHeight(height, _), Some(block_sync)) =
            (&input, &self.block_sync)
        {
            let _ = block_sync
                .cast(BlockSyncMsg::StartHeight(*height))
                .inspect_err(|e| error!("Error when sending start height to BlockSync: {e:?}"));
        }

        malachite_consensus::process!(
            input: input,
            state: &mut state.consensus,
            metrics: &self.metrics,
            with: effect => {
                self.handle_effect(myself, &mut state.timers, &mut state.timeouts, effect).await
            }
        )
    }

    async fn handle_msg(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        msg: Msg<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::StartHeight(height) => {
                let validator_set = self.get_validator_set(height).await?;

                let result = self
                    .process_input(
                        &myself,
                        state,
                        ConsensusInput::StartHeight(height, validator_set),
                    )
                    .await;

                if let Err(e) = result {
                    error!("Error when starting height {height}: {e:?}");
                }

                Ok(())
            }

            Msg::ProposeValue(height, round, value, extension) => {
                let result = self
                    .process_input(
                        &myself,
                        state,
                        ConsensusInput::ProposeValue(height, round, Round::Nil, value, extension),
                    )
                    .await;

                if let Err(e) = result {
                    error!("Error when processing ProposeValue message: {e:?}");
                }

                Ok(())
            }

            Msg::GossipEvent(event) => {
                match event {
                    GossipEvent::Listening(address) => {
                        info!(%address, "Listening");
                    }

                    GossipEvent::PeerConnected(peer_id) => {
                        if !state.connected_peers.insert(peer_id) {
                            // We already saw that peer, ignoring...
                            return Ok(());
                        }

                        info!(%peer_id, "Connected to peer");

                        let validator_set = state.consensus.driver.validator_set();
                        let connected_peers = state.connected_peers.len();
                        let total_peers = validator_set.count() - 1;

                        debug!(connected = %connected_peers, total = %total_peers, "Connected to another peer");

                        self.metrics.connected_peers.inc();

                        // TODO: change logic
                        if connected_peers == total_peers {
                            info!(count = %connected_peers, "Enough peers connected to start consensus");

                            self.host.cast(HostMsg::ConsensusReady(myself.clone()))?;
                        }
                    }

                    GossipEvent::PeerDisconnected(peer_id) => {
                        info!(%peer_id, "Disconnected from peer");

                        if state.connected_peers.remove(&peer_id) {
                            self.metrics.connected_peers.dec();

                            // TODO: pause/stop consensus, if necessary
                        }
                    }

                    GossipEvent::BlockSyncResponse(
                        request_id,
                        peer,
                        blocksync::Response { height, block },
                    ) => {
                        debug!(%height, %request_id, "Received BlockSync response");

                        let Some(block) = block else {
                            error!(%height, %request_id, "Received empty block sync response");
                            return Ok(());
                        };

                        if let Err(e) = self
                            .process_input(
                                &myself,
                                state,
                                ConsensusInput::ReceivedSyncedBlock(
                                    block.block_bytes,
                                    block.certificate,
                                ),
                            )
                            .await
                        {
                            error!(%height, %request_id, "Error when processing received synced block: {e:?}");

                            let Some(block_sync) = self.block_sync.as_ref() else {
                                warn!("Received BlockSync response but BlockSync actor is not available");
                                return Ok(());
                            };

                            if let ConsensusError::InvalidCertificate(certificate, e) = e {
                                block_sync
                                    .cast(BlockSyncMsg::InvalidCertificate(peer, certificate, e))
                                    .map_err(|e| {
                                        eyre!(
                                            "Error when notifying BlockSync of invalid certificate: {e:?}"
                                        )
                                    })?;
                            }
                        }
                    }

                    GossipEvent::Vote(from, vote) => {
                        if let Err(e) = self
                            .process_input(&myself, state, ConsensusInput::Vote(vote))
                            .await
                        {
                            error!(%from, "Error when processing vote: {e:?}");
                        }
                    }

                    GossipEvent::Proposal(from, proposal) => {
                        if state.consensus.params.value_payload.parts_only() {
                            error!(%from, "Properly configured peer should never send proposal messages in BlockPart mode");
                            return Ok(());
                        }

                        if let Err(e) = self
                            .process_input(&myself, state, ConsensusInput::Proposal(proposal))
                            .await
                        {
                            error!(%from, "Error when processing proposal: {e:?}");
                        }
                    }

                    GossipEvent::ProposalPart(from, part) => {
                        if state.consensus.params.value_payload.proposal_only() {
                            error!(%from, "Properly configured peer should never send block part messages in Proposal mode");
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
                                |value| Msg::ReceivedProposedValue(value),
                                None,
                            )
                            .map_err(|e| {
                                eyre!("Error when forwarding proposal parts to host: {e:?}")
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

                state.timeouts.increase_timeout(timeout.step);

                if matches!(timeout.step, TimeoutStep::Prevote | TimeoutStep::Precommit) {
                    warn!(step = ?timeout.step, "Timeout elapsed");

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

            Msg::ReceivedProposedValue(value) => {
                let result = self
                    .process_input(&myself, state, ConsensusInput::ReceivedProposedValue(value))
                    .await;

                if let Err(e) = result {
                    error!("Error when processing GossipEvent message: {e:?}");
                }

                Ok(())
            }

            Msg::GetStatus(reply_to) => {
                let earliest_block_height = self.get_earliest_block_height().await?;
                let status = Status::new(state.consensus.driver.height(), earliest_block_height);

                if let Err(e) = reply_to.send(status) {
                    error!("Error when replying to GetStatus message: {e:?}");
                }

                Ok(())
            }
        }
    }

    fn get_value(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
    ) -> Result<(), ActorProcessingErr> {
        // Call `GetValue` on the Host actor, and forward the reply
        // to the current actor, wrapping it in `Msg::ProposeValue`.
        self.host.call_and_forward(
            |reply| HostMsg::GetValue {
                height,
                round,
                timeout_duration,
                address: self.params.address.clone(),
                reply_to: reply,
            },
            myself,
            |proposed: LocallyProposedValue<Ctx>| {
                Msg::<Ctx>::ProposeValue(
                    proposed.height,
                    proposed.round,
                    proposed.value,
                    proposed.extension,
                )
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

    async fn get_earliest_block_height(&self) -> Result<Ctx::Height, ActorProcessingErr> {
        ractor::call!(self.host, |reply_to| HostMsg::GetEarliestBlockHeight {
            reply_to
        })
        .map_err(|e| eyre!("Failed to get earliest block height: {e:?}").into())
    }

    async fn handle_effect(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        timers: &mut Timers<Ctx>,
        timeouts: &mut Timeouts,
        effect: Effect<Ctx>,
    ) -> Result<Resume<Ctx>, ActorProcessingErr> {
        match effect {
            Effect::ResetTimeouts => {
                timeouts.reset(self.timeout_config);
                Ok(Resume::Continue)
            }

            Effect::CancelAllTimeouts => {
                timers.cancel_all();
                Ok(Resume::Continue)
            }

            Effect::CancelTimeout(timeout) => {
                timers.cancel(&timeout);
                Ok(Resume::Continue)
            }

            Effect::ScheduleTimeout(timeout) => {
                let duration = timeouts.duration_for(timeout.step);
                timers.start_timer(timeout, duration);

                Ok(Resume::Continue)
            }

            Effect::StartRound(height, round, proposer) => {
                self.host.cast(HostMsg::StartedRound {
                    height,
                    round,
                    proposer,
                })?;

                Ok(Resume::Continue)
            }

            Effect::VerifySignature(msg, pk) => {
                use malachite_consensus::ConsensusMsg as Msg;

                let start = Instant::now();

                let valid = match msg.message {
                    Msg::Vote(v) => self.ctx.verify_signed_vote(&v, &msg.signature, &pk),
                    Msg::Proposal(p) => self.ctx.verify_signed_proposal(&p, &msg.signature, &pk),
                };

                self.metrics
                    .signature_verification_time
                    .observe(start.elapsed().as_secs_f64());

                Ok(Resume::SignatureValidity(valid))
            }

            Effect::Broadcast(gossip_msg) => {
                self.gossip_consensus
                    .cast(GossipConsensusMsg::Publish(gossip_msg))
                    .map_err(|e| eyre!("Error when broadcasting gossip message: {e:?}"))?;

                Ok(Resume::Continue)
            }

            Effect::GetValue(height, round, timeout) => {
                let timeout_duration = timeouts.duration_for(timeout.step);

                self.get_value(myself, height, round, timeout_duration)
                    .map_err(|e| eyre!("Error when asking for value to be built: {e:?}"))?;

                Ok(Resume::Continue)
            }

            Effect::GetValidatorSet(height) => {
                let validator_set = self
                    .get_validator_set(height)
                    .await
                    .map_err(|e| warn!("No validator set found for height {height}: {e:?}"))
                    .ok();

                Ok(Resume::ValidatorSet(height, validator_set))
            }

            Effect::RestreamValue(height, round, valid_round, address, value_id) => {
                self.host
                    .cast(HostMsg::RestreamValue {
                        height,
                        round,
                        valid_round,
                        address,
                        value_id,
                    })
                    .map_err(|e| eyre!("Error when sending decided value to host: {e:?}"))?;

                Ok(Resume::Continue)
            }

            Effect::Decide { certificate } => {
                if let Some(tx_decision) = &self.tx_decision {
                    let _ = tx_decision.send(certificate.clone());
                }

                let height = certificate.height;

                self.host
                    .cast(HostMsg::Decided {
                        certificate,
                        consensus: myself.clone(),
                    })
                    .map_err(|e| eyre!("Error when sending decided value to host: {e:?}"))?;

                if let Some(block_sync) = &self.block_sync {
                    block_sync
                        .cast(BlockSyncMsg::Decided(height))
                        .map_err(|e| {
                            eyre!("Error when sending decided height to blocksync: {e:?}")
                        })?;
                }

                Ok(Resume::Continue)
            }

            Effect::SyncedBlock {
                height,
                round,
                validator_address,
                block_bytes,
            } => {
                debug!(%height, "Consensus received synced block, sending to host");

                self.host.call_and_forward(
                    |reply_to| HostMsg::ProcessSyncedBlockBytes {
                        height,
                        round,
                        validator_address,
                        block_bytes,
                        reply_to,
                    },
                    myself,
                    |proposed| Msg::<Ctx>::ReceivedProposedValue(proposed),
                    None,
                )?;

                Ok(Resume::Continue)
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

    async fn pre_start(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        _args: (),
    ) -> Result<State<Ctx>, ActorProcessingErr> {
        let forward = forward(myself.clone(), Some(myself.get_cell()), Msg::GossipEvent).await?;

        self.gossip_consensus
            .cast(GossipConsensusMsg::Subscribe(forward))?;

        Ok(State {
            timers: Timers::new(myself),
            timeouts: Timeouts::new(self.timeout_config),
            consensus: ConsensusState::new(self.ctx.clone(), self.params.clone()),
            connected_peers: BTreeSet::new(),
        })
    }

    async fn post_start(
        &self,
        _myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        state.timers.cancel_all();
        Ok(())
    }

    #[tracing::instrument(
        name = "consensus",
        skip_all,
        fields(
            height = %state.consensus.driver.height(),
            round = %state.consensus.driver.round()
        )
    )]
    async fn handle(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        if let Err(e) = self.handle_msg(myself, state, msg).await {
            error!("Error when handling message: {e:?}");
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        info!("Stopping...");

        state.timers.cancel_all();

        Ok(())
    }
}
