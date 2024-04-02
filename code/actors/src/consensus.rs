use std::collections::VecDeque;
use std::fmt::Display;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use ractor::rpc::call_and_forward;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use malachite_common::{
    Context, Height, NilOrVal, Proposal, Round, SignedProposal, SignedVote, Timeout, TimeoutStep,
    Validator, ValidatorSet, ValueId, Vote, VoteType,
};
use malachite_driver::Input as DriverInput;
use malachite_driver::Output as DriverOutput;
use malachite_driver::Validity;
use malachite_driver::{Driver, ProposerSelector};
use malachite_gossip::{Channel, Event as GossipEvent};
use malachite_network::Msg as NetworkMsg;
use malachite_network::PeerId;
use malachite_proto as proto;
use malachite_proto::Protobuf;
use malachite_vote::{Threshold, ThresholdParams};

use crate::gossip::Msg as GossipMsg;
use crate::proposal_builder::{BuildProposal, ProposedValue};
use crate::timers::{Config as TimersConfig, Msg as TimersMsg, TimeoutElapsed, Timers};
use crate::util::forward;

pub enum Next<Ctx: Context> {
    None,
    Input(DriverInput<Ctx>),
    Decided(Round, Ctx::Value),
}

pub struct Params<Ctx: Context> {
    pub start_height: Ctx::Height,
    pub proposer_selector: Arc<dyn ProposerSelector<Ctx>>,
    pub validator_set: Ctx::ValidatorSet,
    pub address: Ctx::Address,
    pub threshold_params: ThresholdParams,
}

pub struct Consensus<Ctx>
where
    Ctx: Context,
{
    ctx: Ctx,
    params: Params<Ctx>,
    timers_config: TimersConfig,
    gossip: ActorRef<GossipMsg>,
    proposal_builder: ActorRef<BuildProposal<Ctx>>,
    tx_decision: mpsc::Sender<(Ctx::Height, Round, Ctx::Value)>,
}

pub enum Msg<Ctx: Context> {
    StartHeight(Ctx::Height),
    MoveToHeight(Ctx::Height),
    MoveToNextHeight,
    GossipEvent(Arc<GossipEvent>),
    TimeoutElapsed(Timeout),
    ProposeValue(Ctx::Height, Round, Option<Ctx::Value>),
    SendDriverInput(DriverInput<Ctx>),
    Decided(Ctx::Height, Round, Ctx::Value),
    ProcessDriverOutputs(
        Vec<DriverOutput<Ctx>>,
        Option<(VoteType, Round, NilOrVal<ValueId<Ctx>>)>,
    ),
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
    timers: ActorRef<TimersMsg>,
    msg_queue: VecDeque<Msg<Ctx>>,
}

impl<Ctx> Consensus<Ctx>
where
    Ctx: Context,
    Ctx::Vote: Protobuf<Proto = proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = proto::Proposal>,
{
    pub fn new(
        ctx: Ctx,
        params: Params<Ctx>,
        timers_config: TimersConfig,
        gossip: ActorRef<GossipMsg>,
        proposal_builder: ActorRef<BuildProposal<Ctx>>,
        tx_decision: mpsc::Sender<(Ctx::Height, Round, Ctx::Value)>,
    ) -> Self {
        Self {
            ctx,
            params,
            timers_config,
            gossip,
            proposal_builder,
            tx_decision,
        }
    }

    pub async fn spawn(
        ctx: Ctx,
        params: Params<Ctx>,
        timers_config: TimersConfig,
        gossip: ActorRef<GossipMsg>,
        proposal_builder: ActorRef<BuildProposal<Ctx>>,
        tx_decision: mpsc::Sender<(Ctx::Height, Round, Ctx::Value)>,
        supervisor: Option<ActorCell>,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let node = Self::new(
            ctx,
            params,
            timers_config,
            gossip,
            proposal_builder,
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
        match event {
            GossipEvent::Listening(addr) => {
                info!("Listening on {addr}");
            }
            GossipEvent::PeerConnected(peer_id) => {
                info!("Connected to peer {peer_id}");
            }
            GossipEvent::PeerDisconnected(peer_id) => {
                info!("Disconnected from peer {peer_id}");
            }
            GossipEvent::Message(from, Channel::Consensus, data) => {
                let from = PeerId::new(from.to_string());
                let msg = NetworkMsg::from_network_bytes(data).unwrap();

                info!("Received message from peer {from}: {msg:?}");

                self.handle_network_msg(from, msg, myself, state).await?;
            }
        }

        Ok(())
    }

    pub async fn handle_network_msg(
        &self,
        from: PeerId,
        msg: NetworkMsg,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            NetworkMsg::Vote(signed_vote) => {
                let signed_vote = SignedVote::<Ctx>::from_proto(signed_vote).unwrap(); // FIXME
                let validator_address = signed_vote.validator_address();

                info!(%from, %validator_address, "Received vote: {:?}", signed_vote.vote);

                let Some(validator) = self.params.validator_set.get_by_address(validator_address)
                else {
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

                if vote_height > state.driver.height() {
                    warn!(
                        %from, %validator_address,
                        "Received vote for height {0} greater than current height {1}, moving to height {0}",
                        vote_height, state.driver.height(),
                    );

                    // FIXME: We lose the vote here. We should instead buffer it
                    //        and process it once we moved to the correct height.
                    // NOTE: We cannot just send the vote via `SendDriverInput` because otherwise
                    //       the vote will reach the driver before it has started the new height.
                    myself.cast(Msg::MoveToHeight(vote_height))?;

                    return Ok(());
                }

                myself.cast(Msg::SendDriverInput(DriverInput::Vote(signed_vote.vote)))?;
            }

            NetworkMsg::Proposal(proposal) => {
                let signed_proposal = SignedProposal::<Ctx>::from_proto(proposal).unwrap();
                let validator_address = signed_proposal.proposal.validator_address();

                info!(%from, %validator_address, "Received proposal: {:?}", signed_proposal.proposal);

                let Some(validator) = self.params.validator_set.get_by_address(validator_address)
                else {
                    warn!(%from, %validator_address, "Received proposal from unknown validator");
                    return Ok(());
                };

                let valid = self
                    .ctx
                    .verify_signed_proposal(&signed_proposal, validator.public_key());

                let proposal_height = signed_proposal.proposal.height();

                if proposal_height > state.driver.height() {
                    warn!(
                        %from, %validator_address,
                        "Received proposal for height {0} greater than current height {1}, moving to height {0}",
                        proposal_height, state.driver.height(),
                    );

                    // FIXME: We lose the proposal here. We should instead buffer it
                    //        and process it once we moved to the correct height.
                    // NOTE: We cannot just send the proposal via `SendDriverInput` because otherwise
                    //       the proposal will reach the driver before it has started the new height.
                    myself.cast(Msg::MoveToHeight(proposal_height))?;

                    return Ok(());
                }

                myself.cast(Msg::SendDriverInput(DriverInput::Proposal(
                    signed_proposal.proposal,
                    Validity::from_valid(valid),
                )))?;
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
            myself.cast(Msg::MoveToNextHeight)?;
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
            DriverInput::NewRound(_, _) => {
                state.timers.cast(TimersMsg::Reset)?;
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
        }

        let check_threshold = if let DriverInput::Vote(vote) = &input {
            let round = Vote::<Ctx>::round(vote);
            let value = Vote::<Ctx>::value(vote);

            Some((vote.vote_type(), round, value.clone()))
        } else {
            None
        };

        let outputs = state
            .driver
            .process(input)
            .map_err(|e| format!("Driver failed to process input: {e}"))?;

        myself.cast(Msg::ProcessDriverOutputs(outputs, check_threshold))?;

        Ok(())
    }

    async fn process_driver_outputs(
        &self,
        outputs: Vec<DriverOutput<Ctx>>,
        check_threshold: Option<(VoteType, Round, NilOrVal<ValueId<Ctx>>)>,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        // When we receive a vote, check if we've gotten +2/3 votes for the value we just received a vote for,
        // if so then cancel the corresponding timeout.
        if let Some((vote_type, round, value)) = check_threshold {
            let threshold = match value {
                NilOrVal::Nil => Threshold::Nil,
                NilOrVal::Val(value) => Threshold::Value(value),
            };

            let votes = state.driver.votes();

            if votes.is_threshold_met(&round, vote_type, threshold.clone()) {
                let timeout = match vote_type {
                    VoteType::Prevote => Timeout::prevote(round),
                    VoteType::Precommit => Timeout::precommit(round),
                };

                info!("Threshold met for {threshold:?} at round {round}, cancelling {timeout}");
                state.timers.cast(TimersMsg::CancelTimeout(timeout))?;
            }
        }

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
                info!("New round at height {height}: {round}");

                Ok(Next::Input(DriverInput::NewRound(height, round)))
            }

            DriverOutput::Propose(proposal) => {
                info!(
                    "Proposing value {:?} at round {}",
                    proposal.value(),
                    proposal.round()
                );

                let signed_proposal = self.ctx.sign_proposal(proposal);

                // TODO: Refactor to helper method
                let proto = signed_proposal.to_proto().unwrap(); // FIXME
                let msg = NetworkMsg::Proposal(proto);
                let bytes = msg.to_network_bytes().unwrap(); // FIXME
                self.gossip
                    .cast(GossipMsg::Broadcast(Channel::Consensus, bytes))?;

                Ok(Next::Input(DriverInput::Proposal(
                    signed_proposal.proposal,
                    Validity::Valid,
                )))
            }

            DriverOutput::Vote(vote) => {
                info!(
                    "Voting for value {:?} at round {}",
                    vote.value(),
                    vote.round()
                );

                let signed_vote = self.ctx.sign_vote(vote);

                // TODO: Refactor to helper method
                let proto = signed_vote.to_proto().unwrap(); // FIXME
                let msg = NetworkMsg::Vote(proto);
                let bytes = msg.to_network_bytes().unwrap(); // FIXME
                self.gossip
                    .cast(GossipMsg::Broadcast(Channel::Consensus, bytes))?;

                Ok(Next::Input(DriverInput::Vote(signed_vote.vote)))
            }

            DriverOutput::Decide(round, value) => {
                info!("Decided on value {value:?} at round {round}");

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
        let deadline = Instant::now() + self.timers_config.timeout_duration(timeout.step);

        // Call `BuildProposal` on the proposal builder actor,
        // and forward the reply to the current actor, wrapping it in `Msg::ProposeValue`.
        call_and_forward(
            &self.proposal_builder.get_cell(),
            |reply| BuildProposal {
                height,
                round,
                deadline,
                reply,
            },
            myself.get_cell(),
            |proposed: ProposedValue<Ctx>| {
                Msg::<Ctx>::ProposeValue(proposed.height, proposed.round, proposed.value)
            },
            None,
        )?;

        Ok(())
    }
}

#[async_trait]
impl<Ctx> Actor for Consensus<Ctx>
where
    Ctx: Context,
    Ctx::Height: Display,
    Ctx::Vote: Protobuf<Proto = proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = proto::Proposal>,
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
        self.gossip.cast(GossipMsg::Subscribe(forward))?;

        let driver = Driver::new(
            self.ctx.clone(),
            self.params.start_height,
            self.params.proposer_selector.clone(),
            self.params.validator_set.clone(),
            self.params.address.clone(),
            self.params.threshold_params,
        );

        Ok(State {
            driver,
            timers,
            msg_queue: VecDeque::new(),
        })
    }

    #[tracing::instrument(
        name = "node", 
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
                info!("Starting height {height}");

                myself.cast(Msg::SendDriverInput(DriverInput::NewRound(
                    height,
                    Round::new(0),
                )))?;

                // Drain the pending message queue to process any gossip events that were received
                // before the driver started the new height and was still at round Nil.
                let pending_msgs = std::mem::take(&mut state.msg_queue);
                for msg in pending_msgs {
                    myself.cast(msg)?;
                }
            }

            Msg::MoveToNextHeight => {
                let height = state.driver.height().increment();
                info!("Moving to next height {height}");

                myself.cast(Msg::MoveToHeight(height))?;
            }

            Msg::MoveToHeight(height) => {
                state.timers.cast(TimersMsg::Reset)?;
                state.driver.move_to_height(height);

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
                info!("Decided on value {value:?} at height {height} and round {round}");
            }

            Msg::GossipEvent(event) => {
                if state.driver.round() == Round::Nil {
                    debug!("Received gossip event at round -1, queuing for later");
                    state.msg_queue.push_back(Msg::GossipEvent(event));
                } else {
                    self.handle_gossip_event(event.as_ref(), myself, state)
                        .await?;
                }
            }

            Msg::TimeoutElapsed(timeout) => {
                self.handle_timeout(timeout, myself, state).await?;
            }

            Msg::SendDriverInput(input) => {
                self.send_driver_input(input, myself, state).await?;
            }

            Msg::ProcessDriverOutputs(outputs, check_threshold) => {
                self.process_driver_outputs(outputs, check_threshold, myself, state)
                    .await?;
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
