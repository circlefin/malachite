#![allow(dead_code)]

use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::info;

use malachite_common::{
    Context, Proposal, Round, SignedProposal, SignedVote, Timeout, Vote, VoteType,
};
use malachite_driver::{Driver, Input, Output, ProposerSelector, Validity};
use malachite_proto::{self as proto, Protobuf};
use malachite_vote::ThresholdParams;

use crate::network::Msg as NetworkMsg;
use crate::network::Network;
use crate::timers::{self, Timers};

pub struct Params<Ctx: Context> {
    pub start_height: Ctx::Height,
    pub proposer_selector: Arc<dyn ProposerSelector<Ctx>>,
    pub validator_set: Ctx::ValidatorSet,
    pub address: Ctx::Address,
    pub threshold_params: ThresholdParams,
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
    value: Ctx::Value,
}

impl<Ctx, Net> Node<Ctx, Net>
where
    Ctx: Context,
    Net: Network,
    Ctx::Height: Display,
    Ctx::Vote: Protobuf<Proto = proto::Vote>,
    Ctx::Proposal: Protobuf<Proto = proto::Proposal>,
{
    pub fn new(
        ctx: Ctx,
        params: Params<Ctx>,
        network: Net,
        value: Ctx::Value,
        timers_config: timers::Config,
    ) -> Self {
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
            value,
        }
    }

    pub async fn run(mut self) {
        let height = self.driver.height();

        let (tx_input, mut rx_input) = tokio::sync::mpsc::unbounded_channel();
        tx_input
            .send(Input::NewRound(height, Round::new(0)))
            .unwrap();

        loop {
            tokio::select! {
                Some(input) = rx_input.recv() =>{
                    match &input {
                        Input::NewRound(_, _) => {
                            self.timers.reset().await;
                        }
                        Input::ProposeValue(round, _) => {
                            self.timers.cancel_timeout(&Timeout::propose(*round)).await;
                        },
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
                        Input::TimeoutElapsed(_) => (),
                    }

                    let outputs = self.driver.process(input).unwrap();

                    for output in outputs {
                        if let Some(input) = self.process_output(output).await {
                            tx_input.send(input).unwrap();
                        }
                    }
                }

                Some(timeout) = self.timeout_elapsed.recv() => {
                    let height = self.driver.height();
                    let round = self.driver.round();

                    info!("{timeout} elapsed at height {height} and round {round}");

                    tx_input.send(Input::TimeoutElapsed(timeout)).unwrap();
                }

                Some((peer_id, msg)) = self.network.recv() => {
                    info!("Received message from peer {peer_id}: {msg:?}");

                    match msg {
                        NetworkMsg::Vote(signed_vote) => {
                            let signed_vote = SignedVote::<Ctx>::from_proto(signed_vote).unwrap();
                            // self.ctx.verify_signed_vote(signed_vote);
                            tx_input.send(Input::Vote(signed_vote.vote)).unwrap();
                        }
                        NetworkMsg::Proposal(proposal) => {
                            let signed_proposal = SignedProposal::<Ctx>::from_proto(proposal).unwrap();
                            let validity = Validity::Valid; // self.ctx.verify_proposal(proposal);
                            tx_input.send(Input::Proposal(signed_proposal.proposal, validity)).unwrap();
                        }

                        #[cfg(test)]
                        NetworkMsg::Dummy(_) => unreachable!()
                    }
                }
            }
        }
    }

    #[must_use]
    pub async fn process_output(&mut self, output: Output<Ctx>) -> Option<Input<Ctx>> {
        match output {
            Output::NewRound(height, round) => {
                info!("New round at height {height}: {round}");
                Some(Input::NewRound(height, round))
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
                Some(Input::Proposal(signed_proposal.proposal, Validity::Valid))
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
                Some(Input::Vote(signed_vote.vote))
            }

            Output::Decide(round, value) => {
                info!("Decided on value {value:?} at round {round}");
                None
            }

            Output::ScheduleTimeout(timeout) => {
                info!("Scheduling {timeout}");

                self.timers.schedule_timeout(timeout).await;
                None
            }

            Output::GetValue(height, round, _timeout) => {
                info!("Requesting value at height {height} and round {round}");

                let value = self.get_value().await;
                Some(Input::ProposeValue(round, value))
            }
        }
    }

    pub async fn get_value(&self) -> Ctx::Value {
        // Simulate waiting for a value to be assembled
        tokio::time::sleep(Duration::from_secs(1)).await;

        self.value.clone()
    }
}
