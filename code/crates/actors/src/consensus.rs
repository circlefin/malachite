use std::collections::{BTreeSet, VecDeque};
use std::fmt::Display;
use std::sync::Arc;

use async_trait::async_trait;
use ractor::rpc::{call_and_forward, CallResult};
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef};
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};

use malachite_common::{
    Context, Height, Proposal, Round, SignedBlockPart, SignedProposal, SignedVote, Timeout,
    TimeoutStep, Validator, ValidatorSet, Value, Vote,
};
use malachite_driver::Driver;
use malachite_driver::Input as DriverInput;
use malachite_driver::Input::BlockReceived;
use malachite_driver::Output as DriverOutput;
use malachite_driver::Validity;
use malachite_gossip_consensus::{Channel, Event as GossipEvent, PeerId};
use malachite_proto as proto;
use malachite_proto::Protobuf;
use malachite_vote::ThresholdParams;

use crate::gossip_consensus::{GossipConsensusRef, Msg as GossipConsensusMsg};
use crate::host::{HostRef, LocallyProposedValue, Msg as HostMsg, ReceivedProposedValue};
use crate::timers::{Config as TimersConfig, Msg as TimersMsg, TimeoutElapsed, Timers, TimersRef};
use crate::util::forward;

mod network;
use network::NetworkMsg;

mod metrics;
pub use metrics::Metrics;

pub enum Next<Ctx: Context> {
    None,
    Input(DriverInput<Ctx>),
    Decided(Round, Ctx::Value),
}

pub struct ConsensusParams<Ctx: Context> {
    pub start_height: Ctx::Height,
    pub initial_validator_set: Ctx::ValidatorSet,
    pub address: Ctx::Address,
    pub threshold_params: ThresholdParams,
}

pub type ConsensusRef<Ctx> = ActorRef<Msg<Ctx>>;

pub struct Consensus<Ctx>
where
    Ctx: Context,
{
    ctx: Ctx,
    params: ConsensusParams<Ctx>,
    timers_config: TimersConfig,
    gossip_consensus: GossipConsensusRef,
    host: HostRef<Ctx>,
    metrics: Metrics,
    tx_decision: mpsc::Sender<(Ctx::Height, Round, Ctx::Value)>,
}

pub enum Msg<Ctx: Context> {
    StartHeight(Ctx::Height),
    MoveToHeight(Ctx::Height),
    GossipEvent(Arc<GossipEvent>),
    TimeoutElapsed(Timeout),
    SendDriverInput(DriverInput<Ctx>),
    Decided(Ctx::Height, Round, Ctx::Value),
    ProcessDriverOutputs(Vec<DriverOutput<Ctx>>),
    // The proposal builder has built a value and can be used in a new proposal consensus message
    ProposeValue(Ctx::Height, Round, Option<Ctx::Value>),
    // The proposal builder has build a new block part, needs to be signed and gossiped by consensus
    BuilderBlockPart(Ctx::BlockPart),
    BlockReceived(ReceivedProposedValue<Ctx>),
}

impl<Ctx: Context> From<TimeoutElapsed> for Msg<Ctx> {
    fn from(msg: TimeoutElapsed) -> Self {
        Msg::TimeoutElapsed(msg.timeout())
    }
}

pub struct State<Ctx>
where
    Ctx: Context,
{
    driver: Driver<Ctx>,
    timers: TimersRef,
    msg_queue: VecDeque<Msg<Ctx>>,
    validator_set: Ctx::ValidatorSet,
    connected_peers: BTreeSet<PeerId>,
}

impl<Ctx> Consensus<Ctx>
where
    Ctx: Context,
    Ctx::Vote: Protobuf<Proto = proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = proto::Proposal>,
    Ctx::BlockPart: Protobuf<Proto = proto::BlockPart>,
{
    pub fn new(
        ctx: Ctx,
        params: ConsensusParams<Ctx>,
        timers_config: TimersConfig,
        gossip_consensus: GossipConsensusRef,
        host: HostRef<Ctx>,
        metrics: Metrics,
        tx_decision: mpsc::Sender<(Ctx::Height, Round, Ctx::Value)>,
    ) -> Self {
        Self {
            ctx,
            params,
            timers_config,
            gossip_consensus,
            host,
            metrics,
            tx_decision,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        ctx: Ctx,
        params: ConsensusParams<Ctx>,
        timers_config: TimersConfig,
        gossip_consensus: GossipConsensusRef,
        host: HostRef<Ctx>,
        metrics: Metrics,
        tx_decision: mpsc::Sender<(Ctx::Height, Round, Ctx::Value)>,
        supervisor: Option<ActorCell>,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let node = Self::new(
            ctx,
            params,
            timers_config,
            gossip_consensus,
            host,
            metrics,
            tx_decision,
        );

        let (actor_ref, _) = if let Some(supervisor) = supervisor {
            Actor::spawn_linked(None, node, (), supervisor).await?
        } else {
            Actor::spawn(None, node, ()).await?
        };

        Ok(actor_ref)
    }

    pub async fn handle_gossip_event(
        &self,
        event: &GossipEvent,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ractor::ActorProcessingErr> {
        if let GossipEvent::Message(from, _, data) = event {
            let msg = NetworkMsg::from_network_bytes(data).unwrap();

            //info!("Received message from peer {from}: {msg:?}");

            self.handle_network_msg(from, msg, myself, state).await?;
        }

        Ok(())
    }

    pub async fn handle_network_msg(
        &self,
        from: &PeerId,
        msg: NetworkMsg,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            NetworkMsg::Vote(signed_vote) => {
                let signed_vote = SignedVote::<Ctx>::from_proto(signed_vote).unwrap(); // FIXME
                let validator_address = signed_vote.validator_address();

                info!(%from, %validator_address, "Received vote: {:?}", signed_vote.vote);

                let Some(validator) = state.validator_set.get_by_address(validator_address) else {
                    warn!(%from, %validator_address, "Received vote from unknown validator");
                    return Ok(());
                };

                if !self
                    .ctx
                    .verify_signed_vote(&signed_vote, validator.public_key())
                {
                    warn!(%from, %validator_address, "Received invalid vote: {signed_vote:?}");
                    return Ok(());
                }

                let vote_height = signed_vote.vote.height();
                assert!(vote_height == state.driver.height());

                myself.cast(Msg::SendDriverInput(DriverInput::Vote(signed_vote.vote)))?;
            }

            NetworkMsg::Proposal(proposal) => {
                let signed_proposal = SignedProposal::<Ctx>::from_proto(proposal).unwrap();
                let validator_address = signed_proposal.proposal.validator_address();

                info!(%from, %validator_address, "Received proposal: (h: {}, r: {}, id: {:?})",
                    signed_proposal.proposal.height(), signed_proposal.proposal.round(), signed_proposal.proposal.value().id());

                let Some(validator) = state.validator_set.get_by_address(validator_address) else {
                    warn!(%from, %validator_address, "Received proposal from unknown validator");
                    return Ok(());
                };

                // TODO - verify that the proposal was signed by the proposer for the height and round, drop otherwise.
                let proposal = &signed_proposal.proposal;
                let proposal_height = proposal.height();
                let proposal_round = proposal.round();

                if !self
                    .ctx
                    .verify_signed_proposal(&signed_proposal, validator.public_key())
                {
                    error!(
                        "Received invalid signature for proposal ({}, {}, {:?}",
                        proposal_height,
                        proposal_round,
                        proposal.value()
                    );
                    return Ok(());
                }
                assert!(proposal_height == state.driver.height());

                let received_block = state
                    .driver
                    .received_blocks
                    .iter()
                    .find(|&x| x.0 == proposal_height && x.1 == proposal_round);

                match received_block {
                    Some((_height, _round, _value, valid)) => {
                        myself.cast(Msg::SendDriverInput(DriverInput::Proposal(
                            proposal.clone(),
                            *valid,
                        )))?;
                    }
                    None => {
                        // Store the proposal and wait for all block parts
                        // TODO - or maybe integrate with receive-proposal() here? will this block until all parts are received?
                        info!("Received proposal before all block parts, storing it: {proposal:?}",);

                        state.driver.proposal = Some(proposal.clone());
                    }
                }
            }

            NetworkMsg::BlockPart(block_part) => {
                let signed_block_part = SignedBlockPart::<Ctx>::from_proto(block_part).unwrap();
                let validator_address = signed_block_part.validator_address();

                let Some(validator) = state.validator_set.get_by_address(validator_address) else {
                    warn!(%from, %validator_address, "Received block part from unknown validator");
                    return Ok(());
                };

                if !self
                    .ctx
                    .verify_signed_block_part(&signed_block_part, validator.public_key())
                {
                    warn!(%from, %validator_address, "Received invalid block part: {signed_block_part:?}");
                    return Ok(());
                }

                // TODO - verify that the proposal was signed by the proposer for the height and round, drop otherwise.
                self.host.cast(HostMsg::BlockPart {
                    block_part: signed_block_part.block_part,
                    reply_to: myself.clone(),
                })?
            }
        }

        Ok(())
    }

    pub async fn handle_timeout(
        &self,
        timeout: Timeout,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ractor::ActorProcessingErr> {
        let height = state.driver.height();
        let round = state.driver.round();

        if timeout.round != round {
            debug!(
                "Ignoring timeout for round {} at height {}, current round: {round}",
                timeout.round, height
            );

            return Ok(());
        }

        info!("{timeout} elapsed at height {height} and round {round}");

        myself.cast(Msg::SendDriverInput(DriverInput::TimeoutElapsed(timeout)))?;

        if timeout.step == TimeoutStep::Commit {
            myself.cast(Msg::MoveToHeight(height.increment()))?;
        }

        Ok(())
    }

    pub async fn send_driver_input(
        &self,
        input: DriverInput<Ctx>,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match &input {
            DriverInput::NewRound(_, _, _) => {
                state.timers.cast(TimersMsg::CancelAllTimeouts)?;
            }

            DriverInput::ProposeValue(round, _) => state
                .timers
                .cast(TimersMsg::CancelTimeout(Timeout::propose(*round)))?,

            DriverInput::Proposal(proposal, _) => {
                let round = Proposal::<Ctx>::round(proposal);
                state
                    .timers
                    .cast(TimersMsg::CancelTimeout(Timeout::propose(round)))?;
            }

            DriverInput::Vote(_) => (),
            DriverInput::TimeoutElapsed(_) => (),
            DriverInput::BlockReceived(..) => {
                debug!("Received full block {:?}", input);
            }
        }

        let outputs = state
            .driver
            .process(input)
            .map_err(|e| format!("Driver failed to process input: {e}"))?;

        myself.cast(Msg::ProcessDriverOutputs(outputs))?;

        Ok(())
    }

    async fn process_driver_outputs(
        &self,
        outputs: Vec<DriverOutput<Ctx>>,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        for output in outputs {
            let next = self
                .handle_driver_output(output, myself.clone(), state)
                .await?;

            match next {
                Next::None => (),

                Next::Input(input) => myself.cast(Msg::SendDriverInput(input))?,

                Next::Decided(round, value) => {
                    state
                        .timers
                        .cast(TimersMsg::ScheduleTimeout(Timeout::commit(round)))?;

                    myself.cast(Msg::Decided(state.driver.height(), round, value))?;
                }
            }
        }

        Ok(())
    }

    async fn handle_driver_output(
        &self,
        output: DriverOutput<Ctx>,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<Next<Ctx>, ActorProcessingErr> {
        match output {
            DriverOutput::NewRound(height, round) => {
                info!("Starting round {round} at height {height}");

                let validator_set = &state.driver.validator_set;
                let proposer = self.get_proposer(height, round, validator_set).await?;
                info!("Proposer for height {height} and round {round}: {proposer}");

                Ok(Next::Input(DriverInput::NewRound(
                    height,
                    round,
                    proposer.clone(),
                )))
            }

            DriverOutput::Propose(proposal) => {
                info!(
                    "Proposing value with id: {:?}, at round {}",
                    proposal.value().id(),
                    proposal.round()
                );

                let signed_proposal = self.ctx.sign_proposal(proposal);

                // TODO: Refactor to helper method
                let proto = signed_proposal.to_proto().unwrap(); // FIXME
                let msg = NetworkMsg::Proposal(proto);
                let bytes = msg.to_network_bytes().unwrap(); // FIXME
                self.gossip_consensus
                    .cast(GossipConsensusMsg::Broadcast(Channel::Consensus, bytes))?;

                Ok(Next::Input(DriverInput::Proposal(
                    signed_proposal.proposal,
                    Validity::Valid,
                )))
            }

            DriverOutput::Vote(vote) => {
                info!(
                    "Voting {:?} for value {:?} at round {}",
                    vote.vote_type(),
                    vote.value(),
                    vote.round()
                );

                let signed_vote = self.ctx.sign_vote(vote);

                // TODO: Refactor to helper method
                let proto = signed_vote.to_proto().unwrap(); // FIXME
                let msg = NetworkMsg::Vote(proto);
                let bytes = msg.to_network_bytes().unwrap(); // FIXME
                self.gossip_consensus
                    .cast(GossipConsensusMsg::Broadcast(Channel::Consensus, bytes))?;

                Ok(Next::Input(DriverInput::Vote(signed_vote.vote)))
            }

            DriverOutput::Decide(round, value) => {
                info!("Decided on value {:?} at round {round}", value.id());

                let _ = self
                    .tx_decision
                    .send((state.driver.height(), round, value.clone()))
                    .await;

                Ok(Next::Decided(round, value))
            }

            DriverOutput::ScheduleTimeout(timeout) => {
                info!("Scheduling {timeout}");
                state.timers.cast(TimersMsg::ScheduleTimeout(timeout))?;

                Ok(Next::None)
            }

            DriverOutput::GetValue(height, round, timeout) => {
                info!("Requesting value at height {height} and round {round}");
                self.get_value(myself, height, round, timeout).await?;

                Ok(Next::None)
            }
        }
    }

    pub async fn get_value(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        height: Ctx::Height,
        round: Round,
        timeout: Timeout,
    ) -> Result<(), ActorProcessingErr> {
        let timeout_duration = self.timers_config.timeout_duration(timeout.step);

        // Call `GetValue` on the CAL actor, and forward the reply to the current actor,
        // wrapping it in `Msg::ProposeValue`.
        call_and_forward(
            &self.host.get_cell(),
            |reply| HostMsg::GetValue {
                height,
                round,
                timeout_duration,
                address: self.params.address.clone(),
                consensus: myself.clone(),
                reply,
            },
            myself.get_cell(),
            |proposed: LocallyProposedValue<Ctx>| {
                Msg::<Ctx>::ProposeValue(proposed.height, proposed.round, proposed.value)
            },
            None,
        )?;

        Ok(())
    }

    async fn get_proposer(
        &self,
        height: Ctx::Height,
        round: Round,
        validator_set: &Ctx::ValidatorSet,
    ) -> Result<Ctx::Address, ActorProcessingErr> {
        assert!(validator_set.count() > 0);
        assert!(round != Round::Nil && round.as_i64() >= 0);

        let height = height.as_u64() as usize;
        let round = round.as_i64() as usize;

        let proposer_index = (height - 1 + round) % validator_set.count();
        let proposer = validator_set.get_by_index(proposer_index).unwrap();

        Ok(proposer.address().clone())
    }

    async fn get_validator_set(
        &self,
        height: Ctx::Height,
    ) -> Result<Ctx::ValidatorSet, ActorProcessingErr> {
        let result = self
            .host
            .call(
                |reply_to| HostMsg::GetValidatorSet { height, reply_to },
                None,
            )
            .await?;

        // TODO: Figure out better way to handle this:
        // - use `ractor::cast!` macro?
        // - extension trait?
        match result {
            CallResult::Success(validator_set) => Ok(validator_set),
            error => Err(format!(
                "Error at height {height} when waiting for proposer: {error:?}"
            )),
        }
        .map_err(Into::into)
    }
}

#[async_trait]
impl<Ctx> Actor for Consensus<Ctx>
where
    Ctx: Context,
    Ctx::Height: Display,
    Ctx::Vote: Protobuf<Proto = proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = proto::Proposal>,
    Ctx::BlockPart: Protobuf<Proto = proto::BlockPart>,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        _args: (),
    ) -> Result<State<Ctx>, ractor::ActorProcessingErr> {
        let (timers, _) =
            Timers::spawn_linked(self.timers_config, myself.clone(), myself.get_cell()).await?;

        let forward = forward(myself.clone(), Some(myself.get_cell()), Msg::GossipEvent).await?;
        self.gossip_consensus
            .cast(GossipConsensusMsg::Subscribe(forward))?;

        let driver = Driver::new(
            self.ctx.clone(),
            self.params.start_height,
            self.params.initial_validator_set.clone(),
            self.params.address.clone(),
            self.params.threshold_params,
        );

        Ok(State {
            driver,
            timers,
            msg_queue: VecDeque::new(),
            validator_set: self.params.initial_validator_set.clone(),
            connected_peers: BTreeSet::new(),
        })
    }

    #[tracing::instrument(
        name = "consensus",
        skip(self, myself, msg, state),
        fields(
            height = %state.driver.height(),
            round = %state.driver.round()
        )
    )]
    async fn handle(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            Msg::StartHeight(height) => {
                self.metrics.block_start();

                let round = Round::new(0);
                info!("Starting height {height} at round {round}");

                let validator_set = &state.driver.validator_set;
                let proposer = self.get_proposer(height, round, validator_set).await?;
                info!("Proposer for height {height} and round {round}: {proposer}");

                myself.cast(Msg::SendDriverInput(DriverInput::NewRound(
                    height, round, proposer,
                )))?;

                // Drain the pending message queue to process any gossip events that were received
                // before the driver started the new height and was still at round Nil.
                let pending_msgs = std::mem::take(&mut state.msg_queue);
                debug!("Replaying {} messages", pending_msgs.len());
                for msg in pending_msgs {
                    myself.cast(msg)?;
                }
            }

            Msg::MoveToHeight(height) => {
                state.timers.cast(TimersMsg::CancelAllTimeouts)?;
                state.timers.cast(TimersMsg::ResetTimeouts)?;

                let validator_set = self.get_validator_set(height).await?;
                state.driver.move_to_height(height, validator_set);

                debug_assert_eq!(state.driver.height(), height);
                debug_assert_eq!(state.driver.round(), Round::Nil);

                myself.cast(Msg::StartHeight(height))?;
            }

            Msg::ProposeValue(height, round, value) => {
                if state.driver.height() != height {
                    warn!(
                        "Ignoring proposal for height {height}, current height: {}",
                        state.driver.height()
                    );

                    return Ok(());
                }

                if state.driver.round() != round {
                    warn!(
                        "Ignoring proposal for round {round}, current round: {}",
                        state.driver.round()
                    );

                    return Ok(());
                }

                match value {
                    Some(value) => myself.cast(Msg::SendDriverInput(DriverInput::ProposeValue(
                        round, value,
                    )))?,

                    None => warn!(
                        %height, %round,
                        "Proposal builder failed to build a value within the deadline"
                    ),
                }
            }

            Msg::Decided(height, round, value) => {
                info!(
                    "Decided on value {:?} at height {height} and round {round}",
                    value.id()
                );

                self.metrics.block_end();
                self.metrics.finalized_blocks.inc();
                self.metrics
                    .rounds_per_block
                    .observe((round.as_i64() + 1) as f64);
            }

            Msg::GossipEvent(event) => {
                match event.as_ref() {
                    GossipEvent::Listening(addr) => {
                        info!("Listening on {addr}");
                    }

                    GossipEvent::PeerConnected(peer_id) => {
                        info!("Connected to peer {peer_id}");

                        if !state.connected_peers.insert(*peer_id) {
                            // We already saw that peer, ignoring...
                            return Ok(());
                        }

                        self.metrics.connected_peers.inc();

                        if state.connected_peers.len() == state.validator_set.count() - 1 {
                            info!(
                                "Enough peers ({}) connected to start consensus",
                                state.connected_peers.len()
                            );

                            myself.cast(Msg::StartHeight(state.driver.height()))?;
                        }
                    }

                    GossipEvent::PeerDisconnected(peer_id) => {
                        info!("Disconnected from peer {peer_id}");

                        if state.connected_peers.remove(peer_id) {
                            self.metrics.connected_peers.dec();

                            // TODO: pause/stop consensus, if necessary
                        }
                    }

                    GossipEvent::Message(_, _, data) => {
                        let msg = NetworkMsg::from_network_bytes(data).unwrap(); // FIXME

                        let Some(msg_height) = msg.msg_height() else {
                            trace!("Received message without height, dropping");
                            return Ok(());
                        };

                        // Queue messages if driver is not initialized, or if they are for higher height.
                        // Process messages received for the current height.
                        // Drop all others.
                        if state.driver.round() == Round::Nil {
                            debug!("Received gossip event at round -1, queuing for later");
                            state.msg_queue.push_back(Msg::GossipEvent(event));
                        } else if state.driver.height().as_u64() < msg_height {
                            debug!("Received gossip event for higher height, queuing for later");
                            state.msg_queue.push_back(Msg::GossipEvent(event));
                        } else if state.driver.height().as_u64() == msg_height {
                            self.handle_gossip_event(event.as_ref(), myself, state)
                                .await?;
                        }
                    }
                }
            }

            Msg::TimeoutElapsed(timeout) => {
                self.handle_timeout(timeout, myself, state).await?;
            }

            Msg::SendDriverInput(input) => {
                self.send_driver_input(input, myself, state).await?;
            }

            Msg::ProcessDriverOutputs(outputs) => {
                self.process_driver_outputs(outputs, myself, state).await?;
            }

            Msg::BuilderBlockPart(block_part) => {
                let signed_block_part = self.ctx.sign_block_part(block_part);
                let proto = signed_block_part.to_proto().unwrap(); // FIXME
                let msg = NetworkMsg::BlockPart(proto);
                let bytes = msg.to_network_bytes().unwrap(); // FIXME
                self.gossip_consensus
                    .cast(GossipConsensusMsg::Broadcast(Channel::BlockParts, bytes))?;
            }

            Msg::BlockReceived(value) => {
                info!("Received block: {value:?}");

                if let Some(v) = value.value {
                    self.send_driver_input(
                        BlockReceived(value.height, value.round, v, value.valid),
                        myself,
                        state,
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        info!("Stopping...");

        state.timers.stop(None);

        Ok(())
    }
}
