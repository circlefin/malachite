use malachite_common::{Context, Round, Timeout};
use malachite_consensus::executor::{Event, Executor, Message};
use malachite_round::state::{RoundValue, State, Step};

use malachite_test::{
    Address, Height, PrivateKey, Proposal, TestContext, Validator, ValidatorSet, Vote,
};
use rand::rngs::StdRng;
use rand::SeedableRng;

struct TestStep {
    desc: &'static str,
    input_event: Option<Event<TestContext>>,
    expected_output: Option<Message<TestContext>>,
    new_state: State<TestContext>,
}

fn to_input_msg(output: Message<TestContext>) -> Option<Event<TestContext>> {
    match output {
        Message::Propose(p) => Some(Event::Proposal(p)),
        Message::Vote(v) => Some(Event::Vote(v)),
        Message::Decide(_, _) => None,
        Message::SetTimeout(_) => None,
    }
}

#[test]
fn executor_steps_proposer() {
    let value = TestContext::DUMMY_VALUE; // TODO: get value from external source
    let value_id = value.id();

    let mut rng = StdRng::seed_from_u64(0x42);

    let sk1 = PrivateKey::generate(&mut rng);
    let sk2 = PrivateKey::generate(&mut rng);
    let sk3 = PrivateKey::generate(&mut rng);

    let addr1 = Address::from_public_key(&sk1.public_key());
    let addr2 = Address::from_public_key(&sk2.public_key());
    let addr3 = Address::from_public_key(&sk3.public_key());

    let v1 = Validator::new(sk1.public_key(), 1);
    let v2 = Validator::new(sk2.public_key(), 2);
    let v3 = Validator::new(sk3.public_key(), 3);

    let (my_sk, my_addr) = (sk1, addr1);

    let vs = ValidatorSet::new(vec![v1, v2.clone(), v3.clone()]);

    let mut executor = Executor::new(Height::new(1), vs, my_sk.clone(), my_addr);

    let proposal = Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::new(-1));

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are proposer, propose value",
            input_event: Some(Event::NewRound(Round::new(0))),
            expected_output: Some(Message::Propose(proposal.clone())),
            new_state: State {
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive our own proposal, prevote for it (v1)",
            input_event: None,
            expected_output: Some(Message::Vote(
                Vote::new_prevote(Round::new(0), Some(value_id), my_addr).signed(&my_sk),
            )),
            new_state: State {
                round: Round::new(0),
                step: Step::Prevote,
                proposal: Some(proposal.clone()),
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive our own prevote v1",
            input_event: None,
            expected_output: None,
            new_state: State {
                round: Round::new(0),
                step: Step::Prevote,
                proposal: Some(proposal.clone()),
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "v2 prevotes for our proposal",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Round::new(0), Some(value_id), addr2).signed(&sk2),
            )),
            expected_output: None,
            new_state: State {
                round: Round::new(0),
                step: Step::Prevote,
                proposal: Some(proposal.clone()),
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "v3 prevotes for our proposal, we get +2/3 prevotes, precommit for it (v1)",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Round::new(0), Some(value_id), addr3).signed(&sk3),
            )),
            expected_output: Some(Message::Vote(
                Vote::new_precommit(Round::new(0), Some(value_id), my_addr).signed(&my_sk),
            )),
            new_state: State {
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
        TestStep {
            desc: "v1 receives its own precommit",
            input_event: None,
            expected_output: None,
            new_state: State {
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
        TestStep {
            desc: "v2 precommits for our proposal",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Round::new(0), Some(value_id), addr2).signed(&sk2),
            )),
            expected_output: None,
            new_state: State {
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
        TestStep {
            desc: "v3 precommits for our proposal, we get +2/3 precommits, decide it (v1)",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Round::new(0), Some(value_id), addr2).signed(&sk2),
            )),
            expected_output: Some(Message::Decide(Round::new(0), value.clone())),
            new_state: State {
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
        println!("Step: {}", step.desc);

        let execute_message = step
            .input_event
            .unwrap_or_else(|| previous_message.unwrap());

        let output = executor.execute(execute_message);
        assert_eq!(output, step.expected_output);

        let new_state = executor.round_state(Round::new(0)).unwrap();
        assert_eq!(new_state, &step.new_state);

        previous_message = output.and_then(to_input_msg);
    }
}

#[test]
fn executor_steps_not_proposer() {
    let value = TestContext::DUMMY_VALUE; // TODO: get value from external source
    let value_id = value.id();

    let mut rng = StdRng::seed_from_u64(0x42);

    let sk1 = PrivateKey::generate(&mut rng);
    let sk2 = PrivateKey::generate(&mut rng);
    let sk3 = PrivateKey::generate(&mut rng);

    let addr1 = Address::from_public_key(&sk1.public_key());
    let addr2 = Address::from_public_key(&sk2.public_key());
    let addr3 = Address::from_public_key(&sk3.public_key());

    let v1 = Validator::new(sk1.public_key(), 1);
    let v2 = Validator::new(sk2.public_key(), 2);
    let v3 = Validator::new(sk3.public_key(), 3);

    // Proposer is v1, so we are not the proposer
    let (my_sk, my_addr) = (sk2, addr2);

    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);
    let mut executor = Executor::new(Height::new(1), vs, my_sk.clone(), my_addr);

    let proposal = Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::new(-1));

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input_event: Some(Event::NewRound(Round::new(0))),
            expected_output: Some(Message::SetTimeout(Timeout::propose(Round::new(0)))),
            new_state: State {
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive a proposal, prevote for it (v2)",
            input_event: Some(Event::Proposal(proposal.clone())),
            expected_output: Some(Message::Vote(
                Vote::new_prevote(Round::new(0), Some(value_id), my_addr).signed(&my_sk),
            )),
            new_state: State {
                round: Round::new(0),
                step: Step::Prevote,
                proposal: Some(proposal.clone()),
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive our own prevote (v2)",
            input_event: None,
            expected_output: None,
            new_state: State {
                round: Round::new(0),
                step: Step::Prevote,
                proposal: Some(proposal.clone()),
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "v1 prevotes for its own proposal",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Round::new(0), Some(value_id), addr1).signed(&sk1),
            )),
            expected_output: None,
            new_state: State {
                round: Round::new(0),
                step: Step::Prevote,
                proposal: Some(proposal.clone()),
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "v3 prevotes for v1's proposal, it gets +2/3 prevotes, precommit for it (v2)",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Round::new(0), Some(value_id), addr3).signed(&sk3),
            )),
            expected_output: Some(Message::Vote(
                Vote::new_precommit(Round::new(0), Some(value_id), my_addr).signed(&my_sk),
            )),
            new_state: State {
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
        TestStep {
            desc: "we receive our own precommit",
            input_event: None,
            expected_output: None,
            new_state: State {
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
        TestStep {
            desc: "v1 precommits its proposal",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Round::new(0), Some(value_id), addr1).signed(&sk1),
            )),
            expected_output: None,
            new_state: State {
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
        TestStep {
            desc: "v3 precommits for v1's proposal, it gets +2/3 precommits, decide it",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Round::new(0), Some(value_id), addr3).signed(&sk3),
            )),
            expected_output: Some(Message::Decide(Round::new(0), value.clone())),
            new_state: State {
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
        println!("Step: {}", step.desc);

        let execute_message = step
            .input_event
            .unwrap_or_else(|| previous_message.unwrap());

        let output = executor.execute(execute_message);
        assert_eq!(output, step.expected_output);

        let new_state = executor.round_state(Round::new(0)).unwrap();
        assert_eq!(new_state, &step.new_state);

        previous_message = output.and_then(to_input_msg);
    }
}

#[test]
fn executor_steps_not_proposer_timeout() {
    let value = TestContext::DUMMY_VALUE; // TODO: get value from external source
    let value_id = value.id();

    let mut rng = StdRng::seed_from_u64(0x42);

    let sk1 = PrivateKey::generate(&mut rng);
    let sk2 = PrivateKey::generate(&mut rng);
    let sk3 = PrivateKey::generate(&mut rng);

    let addr1 = Address::from_public_key(&sk1.public_key());
    let addr2 = Address::from_public_key(&sk2.public_key());
    let addr3 = Address::from_public_key(&sk3.public_key());

    let v1 = Validator::new(sk1.public_key(), 1);
    let v2 = Validator::new(sk2.public_key(), 1);
    let v3 = Validator::new(sk3.public_key(), 3);

    // Proposer is v1, so we are not the proposer
    let (my_sk, my_addr) = (sk2, addr2);

    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);
    let mut executor = Executor::new(Height::new(1), vs, my_sk.clone(), my_addr);

    let steps = vec![
        // Start round 0, we are not the proposer
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input_event: Some(Event::NewRound(Round::new(0))),
            expected_output: Some(Message::SetTimeout(Timeout::propose(Round::new(0)))),
            new_state: State {
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // Receive a propose timeout, prevote for nil (v1)
        TestStep {
            desc: "Receive a propose timeout, prevote for nil (v1)",
            input_event: Some(Event::Timeout(Timeout::propose(Round::new(0)))),
            expected_output: Some(Message::Vote(
                Vote::new_prevote(Round::new(0), None, my_addr).signed(&my_sk),
            )),
            new_state: State {
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // Receive our own prevote v1
        TestStep {
            desc: "Receive our own prevote v1",
            input_event: None,
            expected_output: None,
            new_state: State {
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v2 prevotes for its own proposal
        TestStep {
            desc: "v2 prevotes for its own proposal",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Round::new(0), Some(value_id), addr1).signed(&sk1),
            )),
            expected_output: None,
            new_state: State {
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v3 prevotes for nil, it gets +2/3 prevotes, precommit for it (v1)
        TestStep {
            desc: "v3 prevotes for nil, it gets +2/3 prevotes, precommit for it (v1)",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Round::new(0), None, addr3).signed(&sk3),
            )),
            expected_output: Some(Message::Vote(
                Vote::new_precommit(Round::new(0), None, my_addr).signed(&my_sk),
            )),
            new_state: State {
                round: Round::new(0),
                step: Step::Precommit,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v1 receives its own precommit
        TestStep {
            desc: "v1 receives its own precommit",
            input_event: None,
            expected_output: None,
            new_state: State {
                round: Round::new(0),
                step: Step::Precommit,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v2 precommits its proposal
        TestStep {
            desc: "v2 precommits its proposal",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Round::new(0), Some(value_id), addr1).signed(&sk1),
            )),
            expected_output: None,
            new_state: State {
                round: Round::new(0),
                step: Step::Precommit,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v3 precommits for nil
        TestStep {
            desc: "v3 precommits for nil",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Round::new(0), None, addr3).signed(&sk3),
            )),
            expected_output: None,
            new_state: State {
                round: Round::new(0),
                step: Step::Precommit,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // we receive a precommit timeout, start a new round
        TestStep {
            desc: "we receive a precommit timeout, start a new round",
            input_event: Some(Event::Timeout(Timeout::precommit(Round::new(0)))),
            expected_output: None,
            new_state: State {
                round: Round::new(1),
                step: Step::NewRound,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
    ];

    let mut previous_message = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_message = step
            .input_event
            .unwrap_or_else(|| previous_message.unwrap());

        let output = executor.execute(execute_message);
        assert_eq!(output, step.expected_output, "expected output message");

        let new_state = executor.round_state(Round::new(0)).unwrap();
        assert_eq!(new_state, &step.new_state, "new state");

        previous_message = output.and_then(to_input_msg);
    }
}
