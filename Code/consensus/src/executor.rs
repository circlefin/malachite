use std::collections::BTreeMap;
use std::sync::Arc;

use malachite_common::{
    Height, Proposal, PublicKey, Round, Timeout, TimeoutStep, ValidatorSet, Value, Vote, VoteType,
};
use malachite_round::events::Event as RoundEvent;
use malachite_round::message::Message as RoundMessage;
use malachite_round::state::State as RoundState;
use malachite_vote::count::Threshold;
use malachite_vote::keeper::VoteKeeper;

#[derive(Clone, Debug)]
pub struct Executor {
    height: Height,
    key: PublicKey,
    validator_set: ValidatorSet,
    round: Round,
    votes: VoteKeeper,
    round_states: BTreeMap<Round, RoundState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    NewRound(Round),
    Proposal(Proposal),
    Vote(Vote),
    Timeout(Timeout),
}

impl Executor {
    pub fn new(height: Height, validator_set: ValidatorSet, key: PublicKey) -> Self {
        let votes = VoteKeeper::new(height, Round::INITIAL, validator_set.total_voting_power());

        Self {
            height,
            key,
            validator_set,
            round: Round::INITIAL,
            votes,
            round_states: BTreeMap::new(),
        }
    }

    pub fn get_value(&self) -> Value {
        // TODO - add external interface to get the value
        Value::new(9999)
    }
    pub fn execute(&mut self, msg: Message) -> Option<Message> {
        let round_msg = match self.apply(msg) {
            Some(msg) => msg,
            None => return None,
        };

        match round_msg {
            RoundMessage::NewRound(round) => {
                // TODO: check if we are the proposer

                self.round_states
                    .insert(round, RoundState::new(self.height).new_round(round));
                None
            }
            RoundMessage::Proposal(p) => {
                // sign the proposal
                Some(Message::Proposal(p))
            }
            RoundMessage::Vote(mut v) => {
                // sign the vote
                // TODO - round message votes should not include address
                v.address = self.validator_set.get_by_public_key(&self.key)?.address();
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

    fn apply(&mut self, msg: Message) -> Option<RoundMessage> {
        match msg {
            Message::NewRound(round) => self.apply_new_round(round),
            Message::Proposal(proposal) => self.apply_proposal(proposal),
            Message::Vote(vote) => self.apply_vote(vote),
            Message::Timeout(timeout) => self.apply_timeout(timeout),
        }
    }

    fn apply_new_round(&mut self, round: Round) -> Option<RoundMessage> {
        let proposer = self.validator_set.get_proposer();
        let event = if proposer.public_key == self.key {
            let value = self.get_value();
            RoundEvent::NewRoundProposer(value)
        } else {
            RoundEvent::NewRound
        };
        self.apply_event(round, event)
    }

    fn apply_proposal(&mut self, proposal: Proposal) -> Option<RoundMessage> {
        // TODO: Check for invalid proposal
        let round = proposal.round;
        let event = RoundEvent::Proposal(proposal.clone());

        let Some(round_state) = self.round_states.get(&self.round) else {
            // TODO: Add logging
            return None;
        };

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

#[cfg(test)]
mod tests {
    use super::*;
    use malachite_common::{Proposal, Validator, Value};
    use malachite_round::state::{RoundValue, State, Step};

    #[test]
    fn test_executor_steps() {
        let value = Value::new(9999); // TODO: get value from external source
        let value_id = value.id();
        let v1 = Validator::new(PublicKey::new(vec![1]), 1);
        let v2 = Validator::new(PublicKey::new(vec![2]), 1);
        let v3 = Validator::new(PublicKey::new(vec![3]), 1);
        let my_address = v1.clone().address();
        let key = v1.clone().public_key; // we are proposer

        let vs = ValidatorSet::new(vec![v1, v2.clone(), v3.clone()]);

        let mut executor = Executor::new(Height::new(1), vs, key.clone());

        let proposal = Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::new(-1));
        struct TestStep {
            input_message: Option<Message>,
            expected_output_message: Option<Message>,
            new_state: State,
        }
        let steps: Vec<TestStep> = vec![
            // Start round 0, we are proposer, propose value
            TestStep {
                input_message: Some(Message::NewRound(Round::new(0))),
                expected_output_message: Some(Message::Proposal(proposal.clone())),
                new_state: State {
                    height: Height::new(1),
                    round: Round::new(0),
                    step: Step::Propose,
                    proposal: None,
                    locked: None,
                    valid: None,
                },
            },
            // Receive our own proposal, prevote for it (v1)
            TestStep {
                input_message: None,
                expected_output_message: Some(Message::Vote(Vote::new_prevote(
                    Round::new(0),
                    Some(value_id),
                    my_address,
                ))),
                new_state: State {
                    height: Height::new(1),
                    round: Round::new(0),
                    step: Step::Prevote,
                    proposal: Some(proposal.clone()),
                    locked: None,
                    valid: None,
                },
            },
            // Receive our own prevote v1
            TestStep {
                input_message: None,
                expected_output_message: None,
                new_state: State {
                    height: Height::new(1),
                    round: Round::new(0),
                    step: Step::Prevote,
                    proposal: Some(proposal.clone()),
                    locked: None,
                    valid: None,
                },
            },
            // v2 prevotes for our proposal
            TestStep {
                input_message: Some(Message::Vote(Vote::new_prevote(
                    Round::new(0),
                    Some(value_id),
                    v2.clone().address(),
                ))),
                expected_output_message: None,
                new_state: State {
                    height: Height::new(1),
                    round: Round::new(0),
                    step: Step::Prevote,
                    proposal: Some(proposal.clone()),
                    locked: None,
                    valid: None,
                },
            },
            // v3 prevotes for our proposal, we get +2/3 prevotes, precommit for it (v1)
            TestStep {
                input_message: Some(Message::Vote(Vote::new_prevote(
                    Round::new(0),
                    Some(value_id),
                    v3.clone().address(),
                ))),
                expected_output_message: Some(Message::Vote(Vote::new_precommit(
                    Round::new(0),
                    Some(value_id),
                    my_address,
                ))),
                new_state: State {
                    height: Height::new(1),
                    round: Round::new(0),
                    step: Step::Precommit,
                    proposal: Some(proposal.clone()),
                    locked: Some(RoundValue {
                        value: value.clone(),
                        round: Round::new(0),
                    }),
                    valid: Some(RoundValue {
                        value: value.clone(),
                        round: Round::new(0),
                    }),
                },
            },
            // v1 receives its own precommit
            TestStep {
                input_message: None,
                expected_output_message: None,
                new_state: State {
                    height: Height::new(1),
                    round: Round::new(0),
                    step: Step::Precommit,
                    proposal: Some(proposal.clone()),
                    locked: Some(RoundValue {
                        value: value.clone(),
                        round: Round::new(0),
                    }),
                    valid: Some(RoundValue {
                        value: value.clone(),
                        round: Round::new(0),
                    }),
                },
            },
            // v2 precommits for our proposal
            TestStep {
                input_message: Some(Message::Vote(Vote::new_precommit(
                    Round::new(0),
                    Some(value_id),
                    v2.clone().address(),
                ))),
                expected_output_message: None,
                new_state: State {
                    height: Height::new(1),
                    round: Round::new(0),
                    step: Step::Precommit,
                    proposal: Some(proposal.clone()),
                    locked: Some(RoundValue {
                        value: value.clone(),
                        round: Round::new(0),
                    }),
                    valid: Some(RoundValue {
                        value: value.clone(),
                        round: Round::new(0),
                    }),
                },
            },
            // v3 precommits for our proposal, we get +2/3 precommits, decide it (v1)
            TestStep {
                input_message: Some(Message::Vote(Vote::new_precommit(
                    Round::new(0),
                    Some(value_id),
                    v2.clone().address(),
                ))),
                expected_output_message: None,
                new_state: State {
                    height: Height::new(1),
                    round: Round::new(0),
                    step: Step::Commit,
                    proposal: Some(proposal.clone()),
                    locked: Some(RoundValue {
                        value: value.clone(),
                        round: Round::new(0),
                    }),
                    valid: Some(RoundValue {
                        value: value.clone(),
                        round: Round::new(0),
                    }),
                },
            },
        ];

        let mut previous_message = None;
        for step in steps {
            let execute_message = if step.input_message.is_none() {
                previous_message.clone()
            } else {
                step.input_message
            }
            .unwrap();
            let message = executor.execute(execute_message);
            assert_eq!(message, step.expected_output_message);
            let new_state = executor.round_states.get(&Round::new(0)).unwrap();
            assert_eq!(new_state, &step.new_state);
            previous_message = message;
        }
    }
}
