use std::fmt::Display;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn, Instrument};

use malachite_common::{
    Context, Height, NilOrVal, Proposal, Round, SignedProposal, SignedVote, Timeout, TimeoutStep,
    Vote, VoteType,
};
use malachite_driver::{Driver, Input, Output, ProposerSelector, Validity};
use malachite_proto::{self as proto, Protobuf};
use malachite_vote::{Threshold, ThresholdParams};

use crate::network::Msg as NetworkMsg;
use crate::network::{Network, PeerId};
use crate::peers::Peers;
use crate::timers::{self, Timers};
use crate::value::ValueBuilder;

pub struct Params<Ctx: Context> {
    pub start_height: Ctx::Height,
    pub proposer_selector: Arc<dyn ProposerSelector<Ctx>>,
    pub proposal_builder: Arc<dyn ValueBuilder<Ctx>>,
    pub validator_set: Ctx::ValidatorSet,
    pub address: Ctx::Address,
    pub threshold_params: ThresholdParams,
    pub peers: Peers<Ctx>,
}

type TxInput<Ctx> = mpsc::UnboundedSender<Input<Ctx>>;

type RxDecision<Ctx> =
    mpsc::UnboundedReceiver<Option<(<Ctx as Context>::Height, Round, <Ctx as Context>::Value)>>;
type TxDecision<Ctx> =
    mpsc::UnboundedSender<Option<(<Ctx as Context>::Height, Round, <Ctx as Context>::Value)>>;

pub struct Handle<Ctx: Context> {
    tx_abort: oneshot::Sender<()>,
    rx_decision: RxDecision<Ctx>,
    _marker: PhantomData<Ctx>,
}

impl<Ctx: Context> Handle<Ctx> {
    pub fn abort(self) {
        self.tx_abort.send(()).unwrap();
    }

    pub async fn wait_decision(&mut self) -> Option<(Ctx::Height, Round, Ctx::Value)> {
        self.rx_decision.recv().await.flatten()
    }
}

pub struct Node<Ctx, Net>
where
    Ctx: Context,
{
    ctx: Ctx,
    driver: Driver<Ctx>,
    params: Params<Ctx>,
    network: Net,
    timers: Timers,
    timeout_elapsed: mpsc::Receiver<Timeout>,
    done: bool,
}

impl<Ctx, Net> Node<Ctx, Net>
where
    Ctx: Context,
    Net: Network,
    Ctx::Height: Display,
    Ctx::Vote: Protobuf<Proto = proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = proto::Proposal>,
{
    pub fn new(ctx: Ctx, params: Params<Ctx>, network: Net, timers_config: timers::Config) -> Self {
        let driver = Driver::new(
            ctx.clone(),
            params.start_height,
            params.proposer_selector.clone(),
            params.validator_set.clone(),
            params.address.clone(),
            params.threshold_params,
        );

        let (timers, timeout_elapsed) = Timers::new(timers_config);

        Self {
            ctx,
            driver,
            params,
            network,
            timers,
            timeout_elapsed,
            done: false,
        }
    }

    pub async fn run(mut self) -> Handle<Ctx> {
        let mut height = self.params.start_height;

        let (tx_abort, mut rx_abort) = oneshot::channel();
        let (tx_decision, rx_decision) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            loop {
                let span = tracing::error_span!("node", height = %height);

                self.start_height(height, &tx_decision)
                    .instrument(span)
                    .await;

                height = self.driver.height().increment();
                self.driver = self.driver.move_to_height(height);

                debug_assert_eq!(self.driver.height(), height);
                debug_assert_eq!(self.driver.round(), Round::Nil);

                if let Ok(()) = rx_abort.try_recv() {
                    break;
                }
            }
        });

        Handle {
            tx_abort,
            rx_decision,
            _marker: PhantomData,
        }
    }

    pub async fn start_height(&mut self, height: Ctx::Height, tx_decision: &TxDecision<Ctx>) {
        let (tx_input, mut rx_input) = mpsc::unbounded_channel();

        tx_input
            .send(Input::NewRound(height, Round::new(0)))
            .unwrap();

        loop {
            if self.done {
                self.done = false;
                self.timers.reset().await;

                break;
            }

            tokio::select! {
                Some(input) = rx_input.recv() => {
                    self.process_input(input, &tx_input, tx_decision).await;
                }

                Some(timeout) = self.timeout_elapsed.recv() => {
                    self.process_timeout(timeout, &tx_input).await;
                }

                Some((peer_id, msg)) = self.network.recv() => {
                    self.process_network_msg(peer_id, msg, &tx_input).await;
                }
            }
        }
    }

    pub async fn process_input(
        &mut self,
        input: Input<Ctx>,
        tx_input: &TxInput<Ctx>,
        tx_decision: &TxDecision<Ctx>,
    ) {
        match &input {
            Input::NewRound(_, _) => {
                self.timers.reset().await;
            }
            Input::ProposeValue(round, _) => {
                self.timers.cancel_timeout(&Timeout::propose(*round)).await;
            }
            Input::Proposal(proposal, _) => {
                let round = Proposal::<Ctx>::round(proposal);
                self.timers.cancel_timeout(&Timeout::propose(round)).await;
            }
            Input::Vote(vote) => {
                // FIXME: Only cancel the timeout when we have received enough* votes
                let round = Vote::<Ctx>::round(vote);
                let timeout = match Vote::<Ctx>::vote_type(vote) {
                    VoteType::Prevote => Timeout::prevote(round),
                    VoteType::Precommit => Timeout::precommit(round),
                };

                self.timers.cancel_timeout(&timeout).await;
            }
            Input::TimeoutElapsed(timeout) if timeout.step == TimeoutStep::Commit => {
                self.done = true;
                return;
            }
            Input::TimeoutElapsed(_) => (),
        }

        let check_threshold = if let Input::Vote(vote) = &input {
            let round = Vote::<Ctx>::round(vote);
            let value = Vote::<Ctx>::value(vote);

            Some((vote.vote_type(), round, value.clone()))
        } else {
            None
        };

        let outputs = self.driver.process(input).unwrap();

        // When we receive a vote, check if we've gotten +2/3 votes for the value we just received a vote for.
        if let Some((vote_type, round, value)) = check_threshold {
            let threshold = match value {
                NilOrVal::Nil => Threshold::Nil,
                NilOrVal::Val(value) => Threshold::Value(value),
            };

            if self
                .driver
                .votes()
                .is_threshold_met(&round, vote_type, threshold.clone())
            {
                let timeout = match vote_type {
                    VoteType::Prevote => Timeout::prevote(round),
                    VoteType::Precommit => Timeout::precommit(round),
                };

                info!("Threshold met for {threshold:?} at round {round}, cancelling {timeout}");
                self.timers.cancel_timeout(&timeout).await;
            }
        }

        for output in outputs {
            match self.process_output(output).await {
                Next::None => (),
                Next::Input(input) => tx_input.send(input).unwrap(),
                Next::Decided(round, value) => {
                    self.timers.schedule_timeout(Timeout::commit(round)).await;

                    tx_decision
                        .send(Some((self.driver.height(), round, value)))
                        .unwrap();
                }
            }
        }
    }

    pub async fn process_timeout(&mut self, timeout: Timeout, tx_input: &TxInput<Ctx>) {
        let height = self.driver.height();
        let round = self.driver.round();

        if timeout.round != round {
            debug!(
                "Ignoring timeout for round {} at height {}, current round: {round}",
                timeout.round, height
            );

            return;
        }

        info!("{timeout} elapsed at height {height} and round {round}");

        tx_input.send(Input::TimeoutElapsed(timeout)).unwrap();
    }

    pub async fn process_network_msg(
        &mut self,
        peer_id: PeerId,
        msg: NetworkMsg,
        tx_input: &TxInput<Ctx>,
    ) {
        info!("Received message from peer {peer_id}: {msg:?}");

        match msg {
            NetworkMsg::Vote(signed_vote) => {
                let signed_vote = SignedVote::<Ctx>::from_proto(signed_vote).unwrap();
                let peer = self.params.peers.get(&peer_id).unwrap(); // FIXME

                if self.ctx.verify_signed_vote(&signed_vote, &peer.public_key) {
                    tx_input.send(Input::Vote(signed_vote.vote)).unwrap();
                } else {
                    warn!("Invalid vote from peer {peer_id}: {signed_vote:?}");
                }
            }
            NetworkMsg::Proposal(proposal) => {
                let signed_proposal = SignedProposal::<Ctx>::from_proto(proposal).unwrap();
                let peer = self.params.peers.get(&peer_id).unwrap(); // FIXME

                let valid = self
                    .ctx
                    .verify_signed_proposal(&signed_proposal, &peer.public_key);

                tx_input
                    .send(Input::Proposal(
                        signed_proposal.proposal,
                        Validity::from_valid(valid),
                    ))
                    .unwrap();
            }

            #[cfg(test)]
            NetworkMsg::Dummy(_) => unreachable!(),
        }
    }

    #[must_use]
    pub async fn process_output(&mut self, output: Output<Ctx>) -> Next<Ctx> {
        match output {
            Output::NewRound(height, round) => {
                info!("New round at height {height}: {round}");
                Next::Input(Input::NewRound(height, round))
            }

            Output::Propose(proposal) => {
                info!(
                    "Proposing value {:?} at round {}",
                    proposal.value(),
                    proposal.round()
                );

                let signed_proposal = self.ctx.sign_proposal(proposal);
                let proto = signed_proposal.to_proto().unwrap();
                self.network.broadcast_proposal(proto).await;
                Next::Input(Input::Proposal(signed_proposal.proposal, Validity::Valid))
            }

            Output::Vote(vote) => {
                info!(
                    "Voting for value {:?} at round {}",
                    vote.value(),
                    vote.round()
                );

                let signed_vote = self.ctx.sign_vote(vote);
                let proto = signed_vote.to_proto().unwrap();
                self.network.broadcast_vote(proto).await;
                Next::Input(Input::Vote(signed_vote.vote))
            }

            Output::Decide(round, value) => {
                info!("Decided on value {value:?} at round {round}");
                self.timers.reset().await;

                // TODO: Wait for `timeout_commit` and start the next height
                Next::Decided(round, value)
            }

            Output::ScheduleTimeout(timeout) => {
                info!("Scheduling {timeout}");
                self.timers.schedule_timeout(timeout).await;

                Next::None
            }

            Output::GetValue(height, round, timeout) => {
                info!("Requesting value at height {height} and round {round}");

                // FIXME: Make this asynchronous, we can't block the event loop
                //        while we are waiting for a value.
                let value = self.get_value(height, &timeout).await;

                Next::Input(Input::ProposeValue(round, value))
            }
        }
    }

    pub async fn get_value(&self, height: Ctx::Height, timeout: &Timeout) -> Ctx::Value {
        let deadline = Instant::now() + self.timers.timeout_duration(&timeout.step);

        self.params
            .proposal_builder
            .build_proposal(height, deadline)
            .await
            .unwrap() // FIXME
    }
}

pub enum Next<Ctx: Context> {
    None,
    Input(Input<Ctx>),
    Decided(Round, Ctx::Value),
}
