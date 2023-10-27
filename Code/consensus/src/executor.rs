use std::collections::BTreeMap;

use secrecy::{ExposeSecret, Secret};

use malachite_common::signature::Keypair;
use malachite_common::{
    Consensus, PrivateKey, Proposal, Round, SignedVote, Timeout, TimeoutStep, Validator,
    ValidatorSet, Value, Vote, VoteType,
};
use malachite_round::events::Event as RoundEvent;
use malachite_round::message::Message as RoundMessage;
use malachite_round::state::State as RoundState;
use malachite_vote::count::Threshold;
use malachite_vote::keeper::VoteKeeper;

use crate::message::Message;

#[derive(Clone, Debug)]
pub struct Executor<C>
where
    C: Consensus,
{
    height: C::Height,
    key: Secret<PrivateKey<C>>,
    validator_set: C::ValidatorSet,
    round: Round,
    votes: VoteKeeper<C>,
    round_states: BTreeMap<Round, RoundState<C>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output<C>
where
    C: Consensus,
{
    Propose(C::Proposal),
    Vote(SignedVote<C>),
    Decide(Round, C::Value),
    SetTimeout(Timeout),
}

impl<C> Executor<C>
where
    C: Consensus,
{
    pub fn new(height: C::Height, validator_set: C::ValidatorSet, key: PrivateKey<C>) -> Self {
        let votes = VoteKeeper::new(
            height.clone(),
            Round::INITIAL,
            validator_set.total_voting_power(),
        );

        Self {
            height,
            key: Secret::new(key),
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

    pub fn execute(&mut self, msg: Message<C>) -> Option<Output<C>> {
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

            RoundMessage::Proposal(proposal) => {
                // sign the proposal
                Some(Output::Propose(proposal))
            }

            RoundMessage::Vote(vote) => {
                let address = self
                    .validator_set
                    .get_by_public_key(&self.key.expose_secret().verifying_key())?
                    .address()
                    .clone();

                let signature = C::sign_vote(&vote, self.key.expose_secret());
                let signed_vote = SignedVote::new(vote, address, signature);

                Some(Output::Vote(signed_vote))
            }

            RoundMessage::Timeout(timeout) => Some(Output::SetTimeout(timeout)),

            RoundMessage::Decision(value) => {
                // TODO: update the state
                Some(Output::Decide(value.round, value.value))
            }
        }
    }

    fn apply(&mut self, msg: Message<C>) -> Option<RoundMessage<C>> {
        match msg {
            Message::NewRound(round) => self.apply_new_round(round),
            Message::Proposal(proposal) => self.apply_proposal(proposal),
            Message::Vote(signed_vote) => self.apply_vote(signed_vote),
            Message::Timeout(timeout) => self.apply_timeout(timeout),
        }
    }

    fn apply_new_round(&mut self, round: Round) -> Option<RoundMessage<C>> {
        let proposer = self.validator_set.get_proposer();

        let event = if proposer.public_key() == &self.key.expose_secret().verifying_key() {
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

    fn apply_vote(&mut self, signed_vote: SignedVote<C>) -> Option<RoundMessage<C>> {
        // TODO: How to handle missing validator?
        let validator = self.validator_set.get_by_address(&signed_vote.address)?;

        if !C::verify_signed_vote(&signed_vote, validator.public_key()) {
            // TODO: How to handle invalid votes?
            return None;
        }

        let round = signed_vote.vote.round();

        let event = self
            .votes
            .apply_vote(signed_vote.vote, validator.voting_power())?;

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
