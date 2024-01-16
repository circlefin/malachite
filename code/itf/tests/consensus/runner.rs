use std::collections::BTreeMap;

use pretty_assertions::assert_eq;

use malachite_common::{Context, NilOrVal, Round, TimeoutStep};
use malachite_itf::consensus::{Input as ModelInput, Output as ModelOutput, State};
use malachite_itf::types::Step;
use malachite_round::input::Input;
use malachite_round::output::Output;
use malachite_round::{state::State as RoundState, state_machine::Info};
use malachite_test::{Address, Height, TestContext};

use itf::Runner as ItfRunner;

use crate::utils::{
    value_from_model, value_from_string, value_id_from_model, value_id_from_string, OTHER_PROCESS,
};

pub struct ConsensusRunner {
    pub address_map: BTreeMap<String, Address>,
}

impl ItfRunner for ConsensusRunner {
    type ActualState = RoundState<TestContext>;
    type Result = Option<Output<TestContext>>;
    type ExpectedState = State;
    type Error = ();

    fn init(&mut self, expected: &Self::ExpectedState) -> Result<Self::ActualState, Self::Error> {
        println!("🔵 init: expected state={:?}", expected.state);

        let height = Height::new(expected.state.height as u64);
        let round = expected.state.round;

        let round = Round::new(round);
        let init_state = RoundState::new(height, round);

        Ok(init_state)
    }

    fn step(
        &mut self,
        actual: &mut Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<Self::Result, Self::Error> {
        println!("🔸 step: actual state={:?}", actual);
        println!("🔸 step: model input={:?}", expected.input);
        println!("🔸 step: model state={:?}", expected.state);

        let address = self.address_map.get(&expected.state.process).unwrap();
        let some_other_node = self.address_map.get(OTHER_PROCESS).unwrap(); // FIXME

        let (data, input) = match &expected.input {
            ModelInput::NoInput => unreachable!(),

            ModelInput::NewRound(round) => {
                let round = Round::new(*round);

                (
                    Info::new(round, address, some_other_node),
                    Input::NewRound(round),
                )
            }

            // TODO: proposal value not used?
            ModelInput::NewRoundProposer(round, _value) => {
                let round = Round::new(*round);
                (Info::new(round, address, address), Input::NewRound(round))
            }

            ModelInput::Proposal(round, value) => {
                let input_round = Round::new(*round);
                let data = Info::new(input_round, address, some_other_node);
                let proposal = TestContext::new_proposal(
                    actual.height,
                    input_round,
                    value_from_model(value).unwrap(),
                    Round::Nil,
                );
                (data, Input::Proposal(proposal))
            }

            ModelInput::ProposalAndPolkaPreviousAndValid(value, valid_round) => {
                let data = Info::new(actual.round, address, some_other_node);
                let proposal = TestContext::new_proposal(
                    actual.height,
                    actual.round,
                    value_from_model(value).unwrap(),
                    Round::new(*valid_round),
                );
                (data, Input::ProposalAndPolkaPrevious(proposal))
            }

            ModelInput::ProposalAndPolkaAndValid(value) => {
                let data = Info::new(actual.round, address, some_other_node);
                let proposal = TestContext::new_proposal(
                    actual.height,
                    actual.round,
                    value_from_model(value).unwrap(),
                    Round::Nil,
                );
                (data, Input::ProposalAndPolkaCurrent(proposal))
            }

            ModelInput::ProposalAndPolkaAndInvalidCInput(height, round, value) => {
                let input_round = Round::new(*round);
                let data = Info::new(input_round, address, some_other_node);
                let proposal = TestContext::new_proposal(
                    Height::new(*height as u64),
                    input_round,
                    value_from_model(value).unwrap(),
                    Round::Nil,
                );
                (data, Input::InvalidProposalAndPolkaPrevious(proposal))
            }

            ModelInput::ProposalAndCommitAndValid(value) => {
                let data = Info::new(actual.round, address, some_other_node);
                let proposal = TestContext::new_proposal(
                    actual.height,
                    actual.round,
                    value_from_model(value).unwrap(),
                    Round::Nil,
                );
                (data, Input::ProposalAndPrecommitValue(proposal))
            }

            ModelInput::ProposalInvalid => (
                Info::new(actual.round, address, some_other_node),
                Input::InvalidProposal,
            ),

            ModelInput::PolkaNil => (
                Info::new(actual.round, address, some_other_node),
                Input::PolkaNil,
            ),

            ModelInput::PolkaAny => (
                Info::new(actual.round, address, some_other_node),
                Input::PolkaAny,
            ),

            ModelInput::PrecommitAny => (
                Info::new(actual.round, address, some_other_node),
                Input::PrecommitAny,
            ),

            ModelInput::RoundSkip(round) => {
                let input_round = Round::new(*round);
                (
                    Info::new(input_round, address, some_other_node),
                    Input::SkipRound(input_round),
                )
            }

            ModelInput::TimeoutPropose(_height, round) => (
                Info::new(Round::new(*round), address, some_other_node),
                Input::TimeoutPropose,
            ),

            ModelInput::TimeoutPrevote(_height, round) => (
                Info::new(Round::new(*round), address, some_other_node),
                Input::TimeoutPrevote,
            ),

            ModelInput::TimeoutPrecommit(_height, round) => (
                Info::new(Round::new(*round), address, some_other_node),
                Input::TimeoutPrecommit,
            ),
        };

        let round_state = core::mem::take(actual);
        let transition = round_state.apply(&data, input);

        println!("🔹 transition: next state={:?}", transition.next_state);
        println!("🔹 transition: output={:?}", transition.output);

        *actual = transition.next_state;

        Ok(transition.output)
    }

    fn result_invariant(
        &self,
        result: &Self::Result,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        // Get expected result.
        let expected_result = &expected.output;

        println!("🟣 result invariant:   actual output={:?}", result);
        println!("🟣 result invariant: expected output={:?}", expected_result);

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
                        value_from_string(&expected_proposal.proposal),
                        "unexpected proposal value"
                    );
                }

                (Output::Vote(vote), ModelOutput::Vote(expected_vote)) => {
                    let expected_src_address = self
                        .address_map
                        .get(expected_vote.src_address.as_str())
                        .unwrap();

                    assert_eq!(vote.validator_address, *expected_src_address);
                    assert_eq!(vote.typ, expected_vote.vote_type.to_common());
                    assert_eq!(vote.height.as_u64() as i64, expected_vote.height);
                    assert_eq!(vote.round.as_i64(), expected_vote.round);

                    let expected_value = expected_vote.value_id.fold(NilOrVal::Nil, |value| {
                        NilOrVal::Val(value_id_from_string(value).unwrap())
                    });
                    assert_eq!(vote.value, expected_value);
                }

                (
                    Output::ScheduleTimeout(timeout),
                    ModelOutput::Timeout(expected_round, expected_timeout),
                ) => {
                    assert_eq!(timeout.round.as_i64(), *expected_round);
                    assert_eq!(timeout.step, expected_timeout.to_common());
                }

                (
                    Output::GetValueAndScheduleTimeout(height, round, timeout),
                    ModelOutput::Proposal(proposal),
                ) => {
                    assert_eq!(height.as_u64(), proposal.height as u64);
                    assert_eq!(round.as_i64(), proposal.round);
                    assert_eq!(timeout.step, TimeoutStep::Propose);

                    // TODO: We need to manually feed `ProposeValue` to the round state
                    //       machine here, and then check the emitted proposal.
                }

                (Output::Decision(decision), ModelOutput::Decided(expected_decided_value)) => {
                    assert_eq!(
                        Some(decision.value),
                        value_from_model(expected_decided_value),
                        "unexpected decided value"
                    );
                }

                _ => panic!("actual: {result:?}\nexpected: {expected:?}"),
            },

            None => panic!("no actual result; expected result: {expected_result:?}"),
        }

        Ok(true)
    }

    fn state_invariant(
        &self,
        actual: &Self::ActualState,
        expected: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        // TODO: What to do with actual.height? There is no height in the spec.

        println!("🟢 state invariant: actual state={:?}", actual);
        println!("🟢 state invariant: expected state={:?}", expected.state);

        if expected.state.step == Step::None {
            // This is the initial state.
            assert_eq!(actual.round, Round::Nil, "unexpected round");
        } else {
            assert_eq!(Some(actual.step), expected.state.step.to_round_step());

            if expected.state.step == Step::Unstarted {
                // In the spec, the new round comes from the input, it's not in the state.
                assert_eq!(
                    actual.round.as_i64(),
                    expected.state.round + 1,
                    "unexpected round"
                );
            } else {
                assert_eq!(
                    actual.round.as_i64(),
                    expected.state.round,
                    "unexpected round"
                );
            }
        }
        assert_eq!(
            actual.valid.as_ref().map(|v| v.round.as_i64()),
            expected.state.valid_round,
            "unexpected valid round"
        );
        assert_eq!(
            actual.valid.as_ref().map(|v| v.value.id()),
            value_id_from_model(&expected.state.valid_value),
            "unexpected valid value"
        );
        assert_eq!(
            actual.locked.as_ref().map(|v| v.round.as_i64()),
            expected.state.locked_round,
            "unexpected locked round"
        );
        assert_eq!(
            actual.locked.as_ref().map(|v| v.value.id()),
            value_id_from_model(&expected.state.locked_value),
            "unexpected locked value"
        );

        Ok(true)
    }
}
