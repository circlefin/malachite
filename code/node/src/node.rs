#![allow(dead_code)]

use std::fmt::Display;
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::info;

use malachite_common::{Context, Proposal, Round, SignedProposal, SignedVote, Timeout, Vote};
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
        }
    }

    pub async fn run(mut self) {
        let height = self.driver.height();
        let mut input = Some(Input::NewRound(height, Round::new(0)));

        loop {
            if let Some(input) = input.take() {
                let outputs = self.driver.process(input).unwrap();

                for output in outputs {
                    self.process_output(output).await;
                }
            }

            tokio::select! {
                Some(timeout) = self.timeout_elapsed.recv() => {
                    let height = self.driver.height();
                    let round = self.driver.round();

                    info!("{timeout:?} elapsed at height {height} and round {round}");

                    input = Some(Input::TimeoutElapsed(timeout));
                }
                Some((peer_id, msg)) = self.network.recv() => {
                    info!("Received message from peer {peer_id}: {msg:?}");

                    match msg {
                        NetworkMsg::Vote(signed_vote) => {
                            let signed_vote = SignedVote::<Ctx>::from_proto(signed_vote).unwrap();
                            // self.ctx.verify_signed_vote(signed_vote);
                            input = Some(Input::Vote(signed_vote.vote));
                        }
                        NetworkMsg::Proposal(proposal) => {
                            let signed_proposal = SignedProposal::<Ctx>::from_proto(proposal).unwrap();
                            let validity = Validity::Valid; // self.ctx.verify_proposal(proposal);
                            input = Some(Input::Proposal(signed_proposal.proposal, validity));
                        }

                        #[cfg(test)]
                        NetworkMsg::Dummy(_) => unreachable!()
                    }
                }
            }
        }
    }

    pub async fn process_output(&mut self, output: Output<Ctx>) {
        match output {
            Output::NewRound(height, round) => {
                info!("New round at height {height}: {round}");
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
            }
            Output::Decide(round, value) => {
                info!("Decided on value {value:?} at round {round}");
            }
            Output::ScheduleTimeout(timeout) => {
                info!("Scheduling {:?} at round {}", timeout.step, timeout.round);

                self.timers.schedule_timeout(timeout).await
            }
            Output::GetValue(height, round, timeout) => {
                info!("Requesting value at height {height} and round {round}");
                info!("Scheduling {:?} at round {}", timeout.step, timeout.round);

                self.timers.schedule_timeout(timeout).await;
            }
        }
    }
}
