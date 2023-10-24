use malachite_common::{Consensus, Round};
use malachite_consensus::executor::{Executor, Message};
use malachite_round::state::{RoundValue, State, Step};

use malachite_test::{Height, Proposal, PublicKey, TestConsensus, Validator, ValidatorSet, Vote};

#[test]
fn test_executor_steps() {
    let value = TestConsensus::DUMMY_VALUE; // TODO: get value from external source
    let value_id = value.id();
    let v1 = Validator::new(PublicKey::new(vec![1]), 1);
    let v2 = Validator::new(PublicKey::new(vec![2]), 1);
    let v3 = Validator::new(PublicKey::new(vec![3]), 1);
    let my_address = v1.address;
    let key = v1.clone().public_key; // we are proposer

    let vs = ValidatorSet::new(vec![v1, v2.clone(), v3.clone()]);

    let mut executor = Executor::new(Height::new(1), vs, key.clone());

    let proposal = Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::new(-1));

    struct TestStep {
        input_message: Option<Message<TestConsensus>>,
        expected_output_message: Option<Message<TestConsensus>>,
        new_state: State<TestConsensus>,
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
                v2.address,
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
                v3.address,
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
                v2.address,
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
                v2.address,
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
        let execute_message = step
            .input_message
            .unwrap_or_else(|| previous_message.unwrap());

        let message = executor.execute(execute_message);
        assert_eq!(message, step.expected_output_message);

        let new_state = executor.round_state(Round::new(0)).unwrap();
        assert_eq!(new_state, &step.new_state);

        previous_message = message;
    }
}
