use std::collections::BTreeMap;
use std::sync::Arc;

use malachite_common::{
    Height, Proposal, Round, Timeout, TimeoutStep, ValidatorSet, Vote, VoteType,
};
use malachite_round::events::Event as RoundEvent;
use malachite_round::message::Message as RoundMessage;
use malachite_round::state::State as RoundState;
use malachite_vote::count::Threshold;
use malachite_vote::keeper::VoteKeeper;

#[derive(Clone, Debug)]
pub struct Executor {
    height: Height,
    validator_set: ValidatorSet,
    round: Round,
    votes: VoteKeeper,
    round_states: BTreeMap<Round, RoundState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Proposal(Proposal),
    Vote(Vote),
    Timeout(Timeout),
}

impl Executor {
    pub fn new(height: Height, validator_set: ValidatorSet) -> Self {
        let votes = VoteKeeper::new(height, Round::INITIAL, validator_set.total_voting_power());

        Self {
            height,
            validator_set,
            round: Round::INITIAL,
            votes,
            round_states: BTreeMap::new(),
        }
    }

    pub fn execute(&mut self, msg: Message) {
        let msg = match self.apply(msg) {
            Some(msg) => msg,
            None => return,
        };

        match msg {
            RoundMessage::NewRound(round) => {
                // TODO: check if we are the proposer

                self.round_states
                    .insert(round, RoundState::new(self.height).new_round(round));
            }
            RoundMessage::Proposal(_) => {
                // sign the proposal
            }
            RoundMessage::Vote(_) => {
                // sign the vote
            }
            RoundMessage::Timeout(_) => {
                // schedule the timeout
            }
            RoundMessage::Decision(_) => {
                // update the state
            }
        }
    }

    fn apply(&mut self, msg: Message) -> Option<RoundMessage> {
        match msg {
            Message::Proposal(proposal) => self.apply_proposal(proposal),
            Message::Vote(vote) => self.apply_vote(vote),
            Message::Timeout(timeout) => self.apply_timeout(timeout),
        }
    }

    fn apply_proposal(&mut self, proposal: Proposal) -> Option<RoundMessage> {
        // TODO: Check for invalid proposal
        let round = proposal.round;
        let event = RoundEvent::Proposal(proposal.clone());
        let round_state = self.round_states.get(&self.round).unwrap();

        if round_state.proposal.is_some() {
            return None;
        }

        if round_state.height != proposal.height || proposal.round != self.round {
            return None;
        }

        if !proposal.pol_round.is_valid()
            || proposal.pol_round.is_defined() && proposal.pol_round >= round_state.round
        {
            return None;
        }

        // TODO verify proposal signature (make some of these checks part of message validation)

        match proposal.pol_round {
            Round::None => {
                // Is it possible to get +2/3 prevotes before the proposal?
                // Do we wait for our own prevote to check the threshold?
                self.apply_event(round, event)
            }
            Round::Some(_)
                if self.votes.check_threshold(
                    &proposal.pol_round,
                    VoteType::Prevote,
                    Threshold::Value(Arc::from(proposal.value.id())),
                ) =>
            {
                self.apply_event(round, event)
            }
            _ => None,
        }
    }

    fn apply_vote(&mut self, vote: Vote) -> Option<RoundMessage> {
        let Some(validator) = self.validator_set.get_by_address(&vote.address) else {
            // TODO: Is this the correct behavior? How to log such "errors"?
            return None;
        };

        let round = vote.round;

        let event = match self.votes.apply_vote(vote, validator.voting_power) {
            Some(event) => event,
            None => return None,
        };

        self.apply_event(round, event)
    }

    fn apply_timeout(&mut self, timeout: Timeout) -> Option<RoundMessage> {
        let event = match timeout.step {
            TimeoutStep::Propose => RoundEvent::TimeoutPropose,
            TimeoutStep::Prevote => RoundEvent::TimeoutPrevote,
            TimeoutStep::Precommit => RoundEvent::TimeoutPrecommit,
        };

        self.apply_event(timeout.round, event)
    }

    /// Apply the event, update the state.
    fn apply_event(&mut self, round: Round, event: RoundEvent) -> Option<RoundMessage> {
        // Get the round state, or create a new one
        let round_state = self
            .round_states
            .remove(&round)
            .unwrap_or_else(|| RoundState::new(self.height));

        // Apply the event to the round state machine
        let transition = round_state.apply_event(round, event);

        // Update state
        self.round_states.insert(round, transition.state);

        // Return message, if any
        transition.message
    }
}
