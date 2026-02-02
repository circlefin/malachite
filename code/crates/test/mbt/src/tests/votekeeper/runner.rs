use std::collections::HashMap;

use malachitebft_core_types::{Context, NilOrVal, Round, SignedVote, Value};
use malachitebft_core_votekeeper::keeper::{Output, VoteKeeper};
use malachitebft_core_votekeeper::ThresholdParams;
use malachitebft_test::{Address, Height, Signature, TestContext, Vote};

use itf::Runner as ItfRunner;
use rand::rngs::StdRng;

use crate::types::{Value as ModelValue, VoteType};
use crate::votekeeper::VoteKeeperOutput::*;
use crate::votekeeper::{State, WeightedVote};

use super::utils::{build_address_map, build_validator_set, check_votes, value_from_model};

#[derive(Debug)]
pub struct VoteKeeperRunner {
    rng: StdRng,
    addresses: HashMap<String, Address>,
    last_state: Option<State>,
    skip_step: bool,
}

impl VoteKeeperRunner {
    pub fn new(rng: StdRng) -> Self {
        Self {
            rng,
            addresses: HashMap::new(),
            last_state: None,
            skip_step: false,
        }
    }
}

impl ItfRunner for VoteKeeperRunner {
    type ActualState = VoteKeeper<TestContext>;
    type Result = Option<Output<<<TestContext as Context>::Value as Value>::Id>>;
    type ExpectedState = State;
    type Error = ();

    fn init(&mut self, expected: &Self::ExpectedState) -> Result<Self::ActualState, Self::Error> {
        let height = expected.bookkeeper.height as u64;
        let total_weight = expected.bookkeeper.total_weight() as u64;
        let validator_weights = &expected.bookkeeper.validator_set;

        println!("🔵 init: height={height}, total_weight={total_weight}");

        let validator_set = build_validator_set(validator_weights.iter(), &mut self.rng);

        let public_keys = validator_weights
            .keys()
            .zip(validator_set.validators.iter())
            .map(|(name, val)| (name, &val.public_key));

        self.addresses = build_address_map(public_keys);

        Ok(VoteKeeper::new(validator_set, ThresholdParams::default()))
    }

    fn step(
        &mut self,
        actual: &mut Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<Self::Result, Self::Error> {
        self.skip_step = false;

        if let Some(last_state) = self.last_state.replace(expected.clone()) {
            if &last_state == expected {
                println!("➡️ Skipping duplicate step");
                self.skip_step = true;
                return Ok(None);
            }
        }

        match &expected.weighted_vote {
            WeightedVote::NoVote => Err(()),

            WeightedVote::Vote(input_vote, weight, current_round) => {
                // Build step to execute.
                let round = Round::from(input_vote.round);
                let height = Height::new(input_vote.height as u64);
                let value = value_from_model(&input_vote.value_id);
                let address = self.addresses.get(input_vote.src_address.as_str()).unwrap();
                let validator = actual.validator_set().get_by_address(address).unwrap();
                let vote = match &input_vote.vote_type {
                    VoteType::Prevote => Vote::new_prevote(height, round, value, *address),
                    VoteType::Precommit => Vote::new_precommit(height, round, value, *address),
                };

                println!(
                    "🔵 step: vote={:?}, round={:?}, value={:?}, validator={:?}, weight={:?}, current_round={:?}",
                    input_vote.vote_type, round, value, input_vote.src_address, weight, current_round
                );

                debug_assert_eq!(*weight as u64, validator.voting_power);

                // Execute step.
                Ok(actual.apply_vote(
                    SignedVote::new(vote, Signature::test()),
                    Round::from(*current_round),
                ))
            }
        }
    }

    fn result_invariant(
        &self,
        result: &Self::Result,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        if self.skip_step {
            return Ok(true);
        }

        let expected_result = &expected.last_emitted;

        match result {
            Some(result) => match (result, expected_result) {
                // TODO: check expected_round
                (Output::PolkaNil, PolkaNil(_expected_round)) => (),
                (Output::PolkaAny, PolkaAny(_expected_round)) => (),
                (Output::PolkaValue(value), PolkaValue(_expected_round, expected_value)) => {
                    assert_eq!(
                        NilOrVal::Val(value),
                        value_from_model(&ModelValue::Val(expected_value.to_string())).as_ref()
                    );
                }
                (Output::PrecommitAny, PrecommitAny(_expected_round)) => (),
                (
                    Output::PrecommitValue(value),
                    PrecommitValue(_expected_round, expected_value),
                ) => {
                    assert_eq!(
                        NilOrVal::Val(value),
                        value_from_model(&ModelValue::Val(expected_value.to_string())).as_ref()
                    );
                }
                (Output::SkipRound(round), Skip(expected_round)) => {
                    assert_eq!(round, &Round::from(*expected_round));
                }
                (actual, expected) => {
                    panic!("actual: {actual:?}, expected: {expected:?}")
                }
            },
            None => assert_eq!(*expected_result, NoOutput),
        }
        Ok(true)
    }

    fn state_invariant(
        &self,
        actual: &Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        if self.skip_step {
            return Ok(true);
        }

        // doesn't check for current Height and Round

        let actual_state = actual;
        let expected_state = &expected.bookkeeper;

        assert_eq!(
            actual_state.total_weight(),
            expected_state.total_weight() as u64,
            "total_weight for the current height"
        );

        assert_eq!(actual_state.rounds(), expected_state.rounds.len());

        for (&round, expected_round) in &expected_state.rounds {
            // doesn't check for current Height and Round

            let actual_round = actual_state.per_round(Round::from(round)).unwrap();

            let expected_outputs = &expected_round.emitted_outputs;
            let actual_outputs = actual_round.emitted_outputs();

            assert_eq!(
                actual_outputs.len(),
                expected_outputs.len(),
                "number of emitted events"
            );

            let mut event_count = HashMap::new();

            for event in expected_outputs {
                let event_name = match event {
                    PolkaAny(_) => "PolkaAny".to_string(),
                    PolkaNil(_) => "PolkaNil".to_string(),
                    PolkaValue(_, _) => "PolkaValue".to_string(),
                    PrecommitAny(_) => "PrecommitAny".to_string(),
                    PrecommitValue(_, _) => "PrecommitValue".to_string(),
                    Skip(_) => "Skip".to_string(),
                    _ => format!("{event:?}"),
                };

                let count = event_count.entry(event_name).or_insert(0);
                *count += 1;
            }

            for event in actual_outputs {
                let event_name = match event {
                    Output::PolkaValue(_) => "PolkaValue".to_string(),
                    Output::PrecommitValue(_) => "PrecommitValue".to_string(),
                    Output::SkipRound(_) => "Skip".to_string(),
                    _ => format!("{event:?}"),
                };

                let count = event_count.entry(event_name).or_insert(0);
                *count -= 1;
            }

            for (event_name, count) in event_count {
                assert_eq!(count, 0, "event {event_name:?} not matched");
            }

            let expected_addresses_weights = &expected_round.votes_addresses_weights;
            let actual_addresses_weights = &actual_round.addresses_weights().get_inner();
            for (address, expected_weight) in expected_addresses_weights {
                assert_eq!(
                    actual_addresses_weights.get(self.addresses.get(address).unwrap()),
                    Some(&(*expected_weight as u64)),
                    "weight for address {address:?}"
                );
            }

            let actual_votes = &actual_round.votes();

            let expected_prevotes = &expected_round.prevotes;
            let actual_prevotes = actual_votes.prevotes();
            check_votes(expected_prevotes, actual_prevotes, &self.addresses);

            let expected_precommits = &expected_round.precommits;
            let actual_precommits = actual_votes.precommits();
            check_votes(expected_precommits, actual_precommits, &self.addresses);
        }

        Ok(true)
    }
}
