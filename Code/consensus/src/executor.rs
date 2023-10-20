use std::collections::BTreeMap;

use malachite_common::{Height, Proposal, Round, Timeout, TimeoutStep, ValidatorSet, Vote};
use malachite_round::events::Event as RoundEvent;
use malachite_round::message::Message as RoundMessage;
use malachite_round::state::State as RoundState;
use malachite_vote::keeper::VoteKeeper;

#[derive(Clone, Debug)]
pub struct Executor {
    height: Height,
    validator_set: ValidatorSet,

    vote_keeper: VoteKeeper,
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
        let vote_keeper = VoteKeeper::new(height, validator_set.total_voting_power());

        Self {
            height,
            validator_set,
            vote_keeper,
            round_states: BTreeMap::new(),
        }
    }

    pub fn execute(&mut self, msg: Message) {
        let msg = match self.apply(msg) {
            Some(msg) => msg,
            None => return,
        };

        match msg {
            RoundMessage::NewRound(_) => {
                // check if we are the proposer
            }
            RoundMessage::Proposal(_) => {
                // sign the proposal
                // call execute
            }
            RoundMessage::Vote(_) => {
                // sign the vote
                // call execute
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
        let event = RoundEvent::Proposal(proposal.value, Round::new(0));
        self.apply_event(proposal.round, event)
    }

    fn apply_vote(&mut self, vote: Vote) -> Option<RoundMessage> {
        // TODO: Get weight
        let weight = 1;

        let round = vote.round;

        let event = match self.vote_keeper.apply_vote(vote, weight) {
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
