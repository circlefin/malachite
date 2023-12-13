use std::collections::HashMap;

use malachite_common::{Context, Round};
use malachite_itf::consensus::{Input as ModelInput, Output as ModelOutput, State as ModelState};
use malachite_itf::types::Address as ModelAddress;
use malachite_round::input::Input;
use malachite_round::output::Output;
use malachite_round::{state::State as RoundState, state_machine::Info};
use malachite_test::{Address, Height, TestContext};

use itf::Runner as ItfRunner;

use crate::utils::{value_from_model, value_from_string, value_id_from_model};

pub struct ConsensusRunner {
    pub address_map: HashMap<String, Address>,
}

impl ItfRunner for ConsensusRunner {
    type ActualState = HashMap<ModelAddress, RoundState<TestContext>>;
    type Result = Option<Output<TestContext>>;
    type ExpectedState = ModelState;
    type Error = ();

    fn init(&mut self, expected: &Self::ExpectedState) -> Result<Self::ActualState, Self::Error> {
        let initial_states_map = &expected.system.0;
        let states_map = initial_states_map
            .iter()
            .map(|(address, state)| {
                let height = state.height;
                let round = state.round as i64;
                println!(
                    "🔵 init: address={:?} height={:?}, round={:?}",
                    address, height, round
                );
                (
                    address.clone(),
                    RoundState::new(Height::new(height as u64), Round::new(round)),
                )
            })
            .collect();
        Ok(states_map)
    }

    #[allow(unused_variables)] // TODO: remove
    fn step(
        &mut self,
        actual: &mut Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<Self::Result, Self::Error> {
        let (input_address, input_event) = &expected.input;
        let address = self.address_map.get(input_address.as_str()).unwrap();
        let proposer = &address.clone(); // FIXME
        let height = &(1 as i64); // FIXME
        let round = &(0 as i64); // FIXME
        let transition = match &input_event {
            ModelInput::NoInput => unreachable!(),
            ModelInput::NewHeight(height) => todo!(),
            ModelInput::NewRound(round) => {
                let input_round = Round::new(*round as i64);
                let data = Info::new(input_round, address, proposer);
                actual
                    .get_mut(input_address)
                    .unwrap()
                    .clone()
                    .apply(&data, Input::NewRound)
            }
            ModelInput::NewRoundProposer(round, value) => {
                let input_round = Round::new(*round as i64);
                let data = Info::new(input_round, address, proposer);
                actual
                    .get_mut(input_address)
                    .unwrap()
                    .clone()
                    .apply(&data, Input::NewRound)
            }
            ModelInput::Proposal(round, value) => {
                let input_height = Height::new(*height as u64);
                let input_round = Round::new(*round as i64);
                let data = Info::new(input_round, address, proposer);
                let proposal = TestContext::new_proposal(
                    input_height,
                    input_round,
                    value_from_model(&value).unwrap(),
                    Round::new(0), // FIXME
                );
                actual
                    .get_mut(input_address)
                    .unwrap()
                    .clone()
                    .apply(&data, Input::Proposal(proposal))
            }
            ModelInput::ProposalAndPolkaPreviousAndValid(round, value) => {
                let input_height = Height::new(*height as u64);
                let input_round = Round::new(*round as i64);
                let data = Info::new(input_round, address, proposer);
                let proposal = TestContext::new_proposal(
                    input_height,
                    input_round,
                    value_from_model(&value).unwrap(),
                    Round::new(0), // FIXME
                );
                actual
                    .get_mut(input_address)
                    .unwrap()
                    .clone()
                    .apply(&data, Input::ProposalAndPolkaPrevious(proposal))
            }
            ModelInput::ProposalAndPolkaAndValid(value) => {
                let input_height = Height::new(*height as u64);
                let input_round = Round::new(*round as i64);
                let data = Info::new(input_round, address, proposer);
                let proposal = TestContext::new_proposal(
                    input_height,
                    input_round,
                    value_from_model(&value).unwrap(),
                    Round::new(0), // FIXME
                );
                actual
                    .get_mut(input_address)
                    .unwrap()
                    .clone()
                    .apply(&data, Input::ProposalAndPolkaCurrent(proposal))
            }
            ModelInput::ProposalAndCommitAndValid(value) => todo!(),
            ModelInput::PolkaNil => {
                let data = Info::new(Round::new(*round as i64), address, proposer);
                actual
                    .get_mut(input_address)
                    .unwrap()
                    .clone()
                    .apply(&data, Input::PolkaNil)
            }
            ModelInput::PolkaAny => {
                let data = Info::new(Round::new(*round as i64), address, proposer);
                actual
                    .get_mut(input_address)
                    .unwrap()
                    .clone()
                    .apply(&data, Input::PolkaAny)
            }
            ModelInput::PrecommitAny => {
                let data = Info::new(Round::new(*round as i64), address, proposer);
                actual
                    .get_mut(input_address)
                    .unwrap()
                    .clone()
                    .apply(&data, Input::PrecommitAny)
            }
            ModelInput::TimeoutPrevote(height, round) => todo!(),
            ModelInput::TimeoutPrecommit(height, round) => todo!(),
            ModelInput::TimeoutPropose(height, round) => todo!(),
            ModelInput::ProposalInvalid => todo!(),
            ModelInput::RoundSkip(round) => todo!(),
            ModelInput::ProposalAndPolkaAndInvalidCInput(height, round, value) => todo!(),
        };
        actual.insert(input_address.clone(), transition.next_state);
        Ok(transition.output)
    }

    fn result_invariant(
        &self,
        result: &Self::Result,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        // Get expected result.
        let expected_result = &expected.output;
        // println!(
        //     "🟣 result: model={:?}({:?},{:?}), code={:?}",
        //     expected_result.name, expected_result.value, expected_result.round, result
        // );
        // Check result against expected result.
        match result {
            Some(result) => match (result, expected_result) {
                (Output::NewRound(round), ModelOutput::SkipRound(expected_round)) => {
                    assert_eq!(round.as_i64(), *expected_round);
                }
                (Output::Proposal(proposal), ModelOutput::Proposal(expected_proposal)) => {
                    // TODO: check expected_proposal.src_address
                    assert_eq!(proposal.height.as_u64() as i64, expected_proposal.height);
                    assert_eq!(proposal.round.as_i64(), expected_proposal.round);
                    assert_eq!(proposal.pol_round.as_i64(), expected_proposal.valid_round);
                    assert_eq!(
                        Some(proposal.value),
                        value_from_string(&expected_proposal.proposal)
                    );
                }
                (Output::Vote(vote), ModelOutput::Vote(expected_vote)) => {
                    let expected_src_address = self
                        .address_map
                        .get(expected_vote.src_address.as_str())
                        .unwrap();
                    assert_eq!(vote.validator_address, *expected_src_address);
                    assert_eq!(
                        format!("{:?}", vote.typ),
                        format!("{:?}", expected_vote.vote_type)
                    );
                    assert_eq!(vote.height.as_u64() as i64, expected_vote.height);
                    assert_eq!(vote.round.as_i64(), expected_vote.round);
                    // assert_eq!(vote.value, value_id_from_model(&expected_vote.value_id));
                }
                (Output::ScheduleTimeout(timeout), ModelOutput::Timeout(expected_timeout)) => {
                    assert_eq!(
                        format!("{:?}", timeout.step),
                        format!("{:?}", expected_timeout)
                    );
                    // assert_eq!("", timeout.round); // FIXME: spec does not have round for timeout
                }
                (
                    Output::GetValueAndScheduleTimeout(_round, timeout),
                    ModelOutput::Timeout(expected_timeout),
                ) => {
                    assert_eq!(
                        format!("{:?}", timeout.step),
                        format!("{:?}", expected_timeout)
                    );
                }
                (Output::Decision(decision), ModelOutput::Decided(expected_decided_value)) => {
                    assert_eq!(
                        Some(decision.value),
                        value_from_model(&expected_decided_value)
                    );
                }
                _ => panic!("actual: {:?}, expected: {:?}", result, expected_result),
            },
            None => panic!("expected_result: {:?}", expected_result),
        }
        Ok(true)
    }

    fn state_invariant(
        &self,
        actual: &Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        // doesn't check for current Height and Round

        let actual_states = actual;
        let expected_states = &expected.system.0;

        assert_eq!(
            actual_states.len(),
            expected_states.len(),
            "number of nodes/processes"
        );

        expected_states.iter().all(|(address, expected)| {
            // doesn't check for current Height and Round
            let actual = actual_states.get(address).unwrap();
            assert_eq!(actual.step, expected.step.to_round_step());
            assert_eq!(actual.round.as_i64(), expected.round);
            assert_eq!(
                actual.valid.as_ref().map(|v| v.round.as_i64()),
                expected.valid_round.map(|vr| vr as i64),
            );
            assert_eq!(
                actual.valid.as_ref().map(|v| v.value.id()),
                value_id_from_model(&expected.valid_value)
            );
            assert_eq!(
                actual.locked.as_ref().map(|v| v.round.as_i64()),
                expected.locked_round.map(|vr| vr as i64)
            );
            assert_eq!(
                actual.locked.as_ref().map(|v| v.value.id()),
                value_id_from_model(&expected.locked_value)
            );
            true
        });

        Ok(true)
    }
}
