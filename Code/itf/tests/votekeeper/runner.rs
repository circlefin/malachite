use std::collections::HashMap;

use malachite_common::{Context, Round, Value};
use malachite_itf::votekeeper::State;
use malachite_test::{Address, Height, TestContext, Vote};
use malachite_vote::{
    keeper::{Message, VoteKeeper},
    ThresholdParams,
};

use itf::Runner as ItfRunner;

use super::utils::{check_votes, value_from_model};

pub struct VoteKeeperRunner {
    pub address_map: HashMap<String, Address>,
}

impl ItfRunner for VoteKeeperRunner {
    type ActualState = VoteKeeper<TestContext>;
    type Result = Option<Message<<<TestContext as Context>::Value as Value>::Id>>;
    type ExpectedState = State;
    type Error = ();

    fn init(&mut self, expected: &Self::ExpectedState) -> Result<Self::ActualState, Self::Error> {
        // Initialize VoteKeeper from the initial total_weight from the first state in the model.
        let (input_vote, weight, current_round) = &expected.weighted_vote;
        let round = Round::new(input_vote.round);
        println!(
            "🔵 init: vote={:?}, round={:?}, value={:?}, address={:?}, weight={:?}, current_round={:?}",
            input_vote.typ, round, input_vote.value, input_vote.address, weight, current_round
        );
        Ok(VoteKeeper::new(
            expected.bookkeeper.total_weight as u64,
            ThresholdParams::default(),
        ))
    }

    fn step(
        &mut self,
        actual: &mut Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<Self::Result, Self::Error> {
        // Build step to execute.
        let (input_vote, weight, current_round) = &expected.weighted_vote;
        let round = Round::new(input_vote.round);
        let height = Height::new(input_vote.height as u64);
        let value = value_from_model(&input_vote.value);
        let address = self.address_map.get(input_vote.address.as_str()).unwrap();
        let vote = match input_vote.typ.as_str() {
            "Prevote" => Vote::new_prevote(height, round, value, *address),
            "Precommit" => Vote::new_precommit(height, round, value, *address),
            _ => unreachable!(),
        };
        println!(
            "🔵 step: vote={:?}, round={:?}, value={:?}, address={:?}, weight={:?}, current_round={:?}",
            input_vote.typ, round, value, input_vote.address, weight, current_round
        );

        // Execute step.
        Ok(actual.apply_vote(vote, *weight as u64, Round::new(*current_round)))
    }

    fn result_invariant(
        &self,
        result: &Self::Result,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        // Get expected result.
        let expected_result = &expected.last_emitted;
        println!(
            "🟣 result: model={:?}({:?},{:?}), code={:?}",
            expected_result.name, expected_result.value, expected_result.round, result
        );
        // Check result against expected result.
        match result {
            Some(result) => match result {
                Message::PolkaValue(value) => {
                    assert_eq!(expected_result.name, "PolkaValue");
                    assert_eq!(
                        value_from_model(&expected_result.value).as_ref(),
                        Some(value)
                    );
                }
                Message::PrecommitValue(value) => {
                    assert_eq!(expected_result.name, "PrecommitValue");
                    assert_eq!(
                        value_from_model(&expected_result.value).as_ref(),
                        Some(value)
                    );
                }
                Message::SkipRound(round) => {
                    assert_eq!(expected_result.name, "Skip");
                    assert_eq!(&Round::new(expected_result.round), round);
                }
                msg => assert_eq!(expected_result.name, format!("{msg:?}")),
            },
            None => assert_eq!(expected_result.name, "None"),
        }
        Ok(true)
    }

    fn state_invariant(
        &self,
        actual: &Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        // doesn't check for current Height and Round

        let actual_state = actual;
        let expected_state = &expected.bookkeeper;

        assert_eq!(
            actual_state.total_weight(),
            &(expected_state.total_weight as u64),
            "total_weight for the current height"
        );

        assert_eq!(actual_state.per_round().len(), expected_state.rounds.len());

        for (&round, expected_round) in &expected_state.rounds {
            // doesn't check for current Height and Round

            let actual_round = actual_state.per_round().get(&Round::new(round)).unwrap();

            let expected_events = &expected_round.emitted_events;
            let actual_events = actual_round.emitted_msgs();

            assert_eq!(
                actual_events.len(),
                expected_events.len(),
                "number of emitted events"
            );

            let mut event_count = HashMap::new();

            for event in expected_events {
                let count = event_count.entry(event.name.clone()).or_insert(0);
                *count += 1;
            }

            for event in actual_events {
                let event_name = match event {
                    Message::PolkaValue(_) => "PolkaValue".into(),
                    Message::PrecommitValue(_) => "PrecommitValue".into(),
                    Message::SkipRound(_) => "Skip".into(),
                    _ => format!("{event:?}"),
                };
                let count = event_count.entry(event_name.clone()).or_insert(0);
                *count -= 1;
            }

            for (event_name, count) in event_count {
                assert_eq!(count, 0, "event {event_name:?} not matched");
            }

            let expected_addresses_weights = &expected_round.votes_addresses_weights;
            let actual_addresses_weights = &actual_round.addresses_weights().get_inner();
            for address in expected_addresses_weights.keys() {
                assert_eq!(
                    actual_addresses_weights.get(self.address_map.get(address).unwrap()),
                    expected_addresses_weights
                        .get(address)
                        .map(|&w| w as u64)
                        .as_ref(),
                    "weight for address {address:?}"
                );
            }

            let actual_votes = &actual_round.votes();

            let expected_prevotes = &expected_round.prevotes;
            let actual_prevotes = actual_votes.prevotes();
            check_votes(expected_prevotes, actual_prevotes, &self.address_map);

            let expected_precommits = &expected_round.precommits;
            let actual_precommits = actual_votes.precommits();
            check_votes(expected_precommits, actual_precommits, &self.address_map);
        }

        Ok(true)
    }
}
