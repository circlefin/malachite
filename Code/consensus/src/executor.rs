use std::collections::BTreeMap;

use malachite_common::{
    Consensus, Proposal, Round, Timeout, TimeoutStep, Validator, ValidatorSet, Value, Vote,
    VoteType,
};
use malachite_round::events::Event as RoundEvent;
use malachite_round::message::Message as RoundMessage;
use malachite_round::state::State as RoundState;
use malachite_vote::count::Threshold;
use malachite_vote::keeper::VoteKeeper;

#[derive(Clone, Debug)]
pub struct Executor<C>
where
    C: Consensus,
{
    height: C::Height,
    key: C::PublicKey,
    validator_set: C::ValidatorSet,
    round: Round,
    votes: VoteKeeper<C>,
    round_states: BTreeMap<Round, RoundState<C>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message<C>
where
    C: Consensus,
{
    NewRound(Round),
    Proposal(C::Proposal),
    Vote(C::Vote),
    Timeout(Timeout),
}

impl<C> Executor<C>
where
    C: Consensus,
{
    pub fn new(height: C::Height, validator_set: C::ValidatorSet, key: C::PublicKey) -> Self {
        let votes = VoteKeeper::new(
            height.clone(),
            Round::INITIAL,
            validator_set.total_voting_power(),
        );

        Self {
            height,
            key,
            validator_set,
            round: Round::INITIAL,
            votes,
            round_states: BTreeMap::new(),
        }
    }

    pub fn get_value(&self) -> C::Value {
        // TODO - add external interface to get the value
        C::DUMMY_VALUE
    }

    pub fn execute(&mut self, msg: Message<C>) -> Option<Message<C>> {
        let round_msg = match self.apply(msg) {
            Some(msg) => msg,
            None => return None,
        };

        match round_msg {
            RoundMessage::NewRound(round) => {
                // TODO: check if we are the proposer

                self.round_states
                    .insert(round, RoundState::new(self.height.clone()).new_round(round));

                None
            }

            RoundMessage::Proposal(p) => {
                // sign the proposal
                Some(Message::Proposal(p))
            }

            RoundMessage::Vote(mut v) => {
                // sign the vote

                // FIXME: round message votes should not include address
                let address = self
                    .validator_set
                    .get_by_public_key(&self.key)?
                    .address()
                    .clone();

                v.set_address(address);

                Some(Message::Vote(v))
            }

            RoundMessage::Timeout(_) => {
                // schedule the timeout
                None
            }

            RoundMessage::Decision(_) => {
                // update the state
                None
            }
        }
    }

    fn apply(&mut self, msg: Message<C>) -> Option<RoundMessage<C>> {
        match msg {
            Message::NewRound(round) => self.apply_new_round(round),
            Message::Proposal(proposal) => self.apply_proposal(proposal),
            Message::Vote(vote) => self.apply_vote(vote),
            Message::Timeout(timeout) => self.apply_timeout(timeout),
        }
    }

    fn apply_new_round(&mut self, round: Round) -> Option<RoundMessage<C>> {
        let proposer = self.validator_set.get_proposer();

        let event = if proposer.public_key() == &self.key {
            let value = self.get_value();
            RoundEvent::NewRoundProposer(value)
        } else {
            RoundEvent::NewRound
        };

        self.apply_event(round, event)
    }

    fn apply_proposal(&mut self, proposal: C::Proposal) -> Option<RoundMessage<C>> {
        // TODO: Check for invalid proposal
        let event = RoundEvent::Proposal(proposal.clone());

        // Check that there is an ongoing round
        let Some(round_state) = self.round_states.get(&self.round) else {
            // TODO: Add logging
            return None;
        };

        // Only process the proposal if there is no other proposal
        if round_state.proposal.is_some() {
            return None;
        }

        // Check that the proposal is for the current height and round
        if round_state.height != proposal.height() || proposal.round() != self.round {
            return None;
        }

        // TODO: Document
        if !proposal.pol_round().is_valid()
            || proposal.pol_round().is_defined() && proposal.pol_round() >= round_state.round
        {
            return None;
        }

        // TODO: Verify proposal signature (make some of these checks part of message validation)
        match proposal.pol_round() {
            Round::Nil => {
                // Is it possible to get +2/3 prevotes before the proposal?
                // Do we wait for our own prevote to check the threshold?
                self.apply_event(proposal.round(), event)
            }
            Round::Some(_)
                if self.votes.is_threshold_met(
                    &proposal.pol_round(),
                    VoteType::Prevote,
                    Threshold::Value(proposal.value().id()),
                ) =>
            {
                self.apply_event(proposal.round(), event)
            }
            _ => None,
        }
    }

    fn apply_vote(&mut self, vote: C::Vote) -> Option<RoundMessage<C>> {
        let Some(validator) = self.validator_set.get_by_address(vote.address()) else {
            // TODO: Is this the correct behavior? How to log such "errors"?
            return None;
        };

        let round = vote.round();

        let event = match self.votes.apply_vote(vote, validator.voting_power()) {
            Some(event) => event,
            None => return None,
        };

        self.apply_event(round, event)
    }

    fn apply_timeout(&mut self, timeout: Timeout) -> Option<RoundMessage<C>> {
        let event = match timeout.step {
            TimeoutStep::Propose => RoundEvent::TimeoutPropose,
            TimeoutStep::Prevote => RoundEvent::TimeoutPrevote,
            TimeoutStep::Precommit => RoundEvent::TimeoutPrecommit,
        };

        self.apply_event(timeout.round, event)
    }

    /// Apply the event, update the state.
    fn apply_event(&mut self, round: Round, event: RoundEvent<C>) -> Option<RoundMessage<C>> {
        // Get the round state, or create a new one
        let round_state = self
            .round_states
            .remove(&round)
            .unwrap_or_else(|| RoundState::new(self.height.clone()));

        // Apply the event to the round state machine
        let transition = round_state.apply_event(round, event);

        // Update state
        self.round_states.insert(round, transition.state);

        // Return message, if any
        transition.message
    }

    pub fn round_state(&self, round: Round) -> Option<&RoundState<C>> {
        self.round_states.get(&round)
    }
}
