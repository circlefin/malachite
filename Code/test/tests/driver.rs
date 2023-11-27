use futures::executor::block_on;
use malachite_test::utils::{make_validators, FixedProposer, RotateProposer};

use malachite_common::{Round, Timeout, TimeoutStep};
use malachite_driver::{Driver, Error, Event, Message, Validity};
use malachite_round::state::{RoundValue, State, Step};
use malachite_test::{Height, Proposal, TestContext, ValidatorSet, Value, Vote};

pub struct TestStep {
    desc: &'static str,
    input_event: Option<Event<TestContext>>,
    expected_output: Option<Message<TestContext>>,
    expected_round: Round,
    new_state: State<TestContext>,
}

pub fn msg_to_event(output: Message<TestContext>) -> Option<Event<TestContext>> {
    match output {
        Message::NewRound(height, round) => Some(Event::NewRound(height, round)),
        // Let's consider our own proposal to always be valid
        Message::Propose(p) => Some(Event::Proposal(p, Validity::Valid)),
        Message::Vote(v) => Some(Event::Vote(v)),
        Message::Decide(_, _) => None,
        Message::ScheduleTimeout(_) => None,
        Message::GetValueAndScheduleTimeout(_, _) => None,
    }
}

#[test]
fn driver_steps_proposer() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([1, 2, 3]);
    let (my_sk, my_addr) = (sk1, v1.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = FixedProposer::new(my_addr);
    let vs = ValidatorSet::new(vec![v1, v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let proposal = Proposal::new(Height::new(1), Round::new(0), value, Round::new(-1));

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are proposer, ask for a value to propose",
            input_event: Some(Event::NewRound(Height::new(1), Round::new(0))),
            expected_output: Some(Message::GetValueAndScheduleTimeout(
                Round::new(0),
                Timeout::new(Round::new(0), TimeoutStep::Propose),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Feed a value to propose, propose that value",
            input_event: Some(Event::ProposeValue(Round::new(0), value)),
            expected_output: Some(Message::Propose(proposal.clone())),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
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
                Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), my_addr)
                    .signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
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
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
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
                Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), v2.address)
                    .signed(&sk2),
            )),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
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
                Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), v3.address)
                    .signed(&sk3),
            )),
            expected_output: Some(Message::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), Some(value.id()), my_addr)
                    .signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: Some(proposal.clone()),
                locked: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
            },
        },
        TestStep {
            desc: "v1 receives its own precommit",
            input_event: None,
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: Some(proposal.clone()),
                locked: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
            },
        },
        TestStep {
            desc: "v2 precommits for our proposal",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), Some(value.id()), v2.address)
                    .signed(&sk2),
            )),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: Some(proposal.clone()),
                locked: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
            },
        },
        TestStep {
            desc: "v3 precommits for our proposal, we get +2/3 precommits, decide it (v1)",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), Some(value.id()), v3.address)
                    .signed(&sk3),
            )),
            expected_output: Some(Message::Decide(Round::new(0), value)),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Commit,
                proposal: Some(proposal.clone()),
                locked: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
            },
        },
    ];

    let mut event_from_previous_msg = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_event = step
            .input_event
            .unwrap_or_else(|| event_from_previous_msg.unwrap());

        let output = block_on(driver.execute(execute_event)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output message");

        assert_eq!(
            driver.round_state.round, step.expected_round,
            "expected round"
        );

        assert_eq!(driver.round_state, step.new_state, "expected state");

        event_from_previous_msg = output.and_then(msg_to_event);
    }
}

#[test]
fn driver_steps_proposer_timeout_get_value() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (my_sk, my_addr) = (sk1, v1.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = FixedProposer::new(my_addr);
    let vs = ValidatorSet::new(vec![v1, v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are proposer, ask for a value to propose",
            input_event: Some(Event::NewRound(Height::new(1), Round::new(0))),
            expected_output: Some(Message::GetValueAndScheduleTimeout(
                Round::new(0),
                Timeout::new(Round::new(0), TimeoutStep::Propose),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive a propose timeout",
            input_event: Some(Event::TimeoutElapsed(Timeout::propose(Round::new(0)))),
            expected_output: Some(Message::Vote(
                Vote::new_prevote(Height::new(1), Round::new(0), None, my_addr).signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                round: Round::new(0),
                height: Height::new(1),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
    ];

    let mut event_from_previous_msg = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_event = step
            .input_event
            .unwrap_or_else(|| event_from_previous_msg.unwrap());

        let output = block_on(driver.execute(execute_event)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output message");

        assert_eq!(
            driver.round_state.round, step.expected_round,
            "expected round"
        );

        assert_eq!(driver.round_state, step.new_state, "expected state");

        event_from_previous_msg = output.and_then(msg_to_event);
    }
}

#[test]
fn driver_steps_not_proposer_valid() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([1, 2, 3]);

    // Proposer is v1, so we are not the proposer
    let (my_sk, my_addr) = (sk2, v2.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = FixedProposer::new(v1.address);
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let proposal = Proposal::new(Height::new(1), Round::new(0), value, Round::new(-1));

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input_event: Some(Event::NewRound(Height::new(1), Round::new(0))),
            expected_output: Some(Message::ScheduleTimeout(Timeout::propose(Round::new(0)))),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive a proposal, prevote for it (v2)",
            input_event: Some(Event::Proposal(proposal.clone(), Validity::Valid)),
            expected_output: Some(Message::Vote(
                Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), my_addr)
                    .signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
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
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
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
                Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), v1.address)
                    .signed(&sk1),
            )),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
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
                Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), v3.address)
                    .signed(&sk3),
            )),
            expected_output: Some(Message::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), Some(value.id()), my_addr)
                    .signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: Some(proposal.clone()),
                locked: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
            },
        },
        TestStep {
            desc: "we receive our own precommit",
            input_event: None,
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: Some(proposal.clone()),
                locked: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
            },
        },
        TestStep {
            desc: "v1 precommits its proposal",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), Some(value.id()), v1.address)
                    .signed(&sk1),
            )),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: Some(proposal.clone()),
                locked: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
            },
        },
        TestStep {
            desc: "v3 precommits for v1's proposal, it gets +2/3 precommits, decide it",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), Some(value.id()), v3.address)
                    .signed(&sk3),
            )),
            expected_output: Some(Message::Decide(Round::new(0), value)),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Commit,
                proposal: Some(proposal.clone()),
                locked: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value,
                    round: Round::new(0),
                }),
            },
        },
    ];

    let mut event_from_previous_msg = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_event = step
            .input_event
            .unwrap_or_else(|| event_from_previous_msg.unwrap());

        let output = block_on(driver.execute(execute_event)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output message");

        assert_eq!(
            driver.round_state.round, step.expected_round,
            "expected round"
        );

        assert_eq!(driver.round_state, step.new_state, "expected state");

        event_from_previous_msg = output.and_then(msg_to_event);
    }
}

#[test]
fn driver_steps_not_proposer_invalid() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([1, 2, 3]);

    // Proposer is v1, so we are not the proposer
    let (my_sk, my_addr) = (sk2, v2.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = FixedProposer::new(v1.address);
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let proposal = Proposal::new(Height::new(1), Round::new(0), value, Round::new(-1));

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input_event: Some(Event::NewRound(Height::new(1), Round::new(0))),
            expected_output: Some(Message::ScheduleTimeout(Timeout::propose(Round::new(0)))),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive an invalid proposal, prevote for nil (v2)",
            input_event: Some(Event::Proposal(proposal.clone(), Validity::Invalid)),
            expected_output: Some(Message::Vote(
                Vote::new_prevote(Height::new(1),Round::new(0), None, my_addr).signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive our own prevote (v2)",
            input_event: None,
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "v1 prevotes for its own proposal",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), v1.address).signed(&sk1),
            )),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "v3 prevotes for v1's proposal, we have polka for any, schedule prevote timeout (v2)",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), v3.address).signed(&sk3),
            )),
            expected_output: Some(Message::ScheduleTimeout(Timeout::prevote(Round::new(0)))),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "prevote timeout elapses, we precommit for nil (v2)",
            input_event: Some(Event::TimeoutElapsed(Timeout::prevote(Round::new(0)))),
            expected_output: Some(Message::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), None, my_addr).signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
    ];

    let mut event_from_previous_msg = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_event = step
            .input_event
            .unwrap_or_else(|| event_from_previous_msg.unwrap());

        let output = block_on(driver.execute(execute_event)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output");

        assert_eq!(
            driver.round_state.round, step.expected_round,
            "expected round"
        );

        assert_eq!(driver.round_state, step.new_state, "expected state");

        event_from_previous_msg = output.and_then(msg_to_event);
    }
}

#[test]
fn driver_steps_not_proposer_other_height() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2)] = make_validators([1, 2]);

    // Proposer is v1, so we are not the proposer
    let (my_sk, my_addr) = (sk2, v2.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = FixedProposer::new(v1.address);
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    // Proposal is for another height
    let proposal = Proposal::new(Height::new(2), Round::new(0), value, Round::new(-1));

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input_event: Some(Event::NewRound(Height::new(1), Round::new(0))),
            expected_output: Some(Message::ScheduleTimeout(Timeout::propose(Round::new(0)))),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive a proposal for another height, ignore it (v2)",
            input_event: Some(Event::Proposal(proposal.clone(), Validity::Invalid)),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
    ];

    let mut event_from_previous_msg = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_event = step
            .input_event
            .unwrap_or_else(|| event_from_previous_msg.unwrap());

        let output = block_on(driver.execute(execute_event)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output");

        assert_eq!(
            driver.round_state.round, step.expected_round,
            "expected round"
        );

        assert_eq!(driver.round_state, step.new_state, "expected state");

        event_from_previous_msg = output.and_then(msg_to_event);
    }
}

#[test]
fn driver_steps_not_proposer_other_round() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2)] = make_validators([1, 2]);

    // Proposer is v1, so we are not the proposer
    let (my_sk, my_addr) = (sk2, v2.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = FixedProposer::new(v1.address);
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    // Proposal is for another round
    let proposal = Proposal::new(Height::new(1), Round::new(1), value, Round::new(-1));

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input_event: Some(Event::NewRound(Height::new(1), Round::new(0))),
            expected_output: Some(Message::ScheduleTimeout(Timeout::propose(Round::new(0)))),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Receive a proposal for another round, ignore it (v2)",
            input_event: Some(Event::Proposal(proposal.clone(), Validity::Invalid)),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
    ];

    let mut event_from_previous_msg = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_event = step
            .input_event
            .unwrap_or_else(|| event_from_previous_msg.unwrap());

        let output = block_on(driver.execute(execute_event)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output");

        assert_eq!(
            driver.round_state.round, step.expected_round,
            "expected round"
        );

        assert_eq!(driver.round_state, step.new_state, "expected state");

        event_from_previous_msg = output.and_then(msg_to_event);
    }
}

#[test]
fn driver_steps_not_proposer_timeout_multiple_rounds() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([1, 3, 1]);

    // Proposer is v1, so we, v3, are not the proposer
    let (my_sk, my_addr) = (sk3, v3.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = FixedProposer::new(v1.address);
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let steps = vec![
        // Start round 0, we, v3, are not the proposer
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer",
            input_event: Some(Event::NewRound(Height::new(1), Round::new(0))),
            expected_output: Some(Message::ScheduleTimeout(Timeout::propose(Round::new(0)))),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // Receive a propose timeout, prevote for nil (from v3)
        TestStep {
            desc: "Receive a propose timeout, prevote for nil (v3)",
            input_event: Some(Event::TimeoutElapsed(Timeout::propose(Round::new(0)))),
            expected_output: Some(Message::Vote(
                Vote::new_prevote(Height::new(1), Round::new(0), None, my_addr).signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                round: Round::new(0),
                height: Height::new(1),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // Receive our own prevote v3
        TestStep {
            desc: "Receive our own prevote v3",
            input_event: None,
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v1 prevotes for its own proposal
        TestStep {
            desc: "v1 prevotes for its own proposal",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), v1.address)
                    .signed(&sk1),
            )),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v2 prevotes for nil, we get +2/3 nil prevotes and precommit for nil
        TestStep {
            desc: "v2 prevotes for nil, we get +2/3 prevotes, precommit for nil",
            input_event: Some(Event::Vote(
                Vote::new_prevote(Height::new(1), Round::new(0), None, v2.address).signed(&sk2),
            )),
            expected_output: Some(Message::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), None, my_addr).signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v3 receives its own precommit
        TestStep {
            desc: "v3 receives its own precommit",
            input_event: None,
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v1 precommits its proposal
        TestStep {
            desc: "v1 precommits its proposal",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), Some(value.id()), v1.address)
                    .signed(&sk1),
            )),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v2 precommits for nil
        TestStep {
            desc: "v2 precommits for nil",
            input_event: Some(Event::Vote(
                Vote::new_precommit(Height::new(1), Round::new(0), None, v2.address).signed(&sk2),
            )),
            expected_output: Some(Message::ScheduleTimeout(Timeout::precommit(Round::new(0)))),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
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
            input_event: Some(Event::TimeoutElapsed(Timeout::precommit(Round::new(0)))),
            expected_output: Some(Message::NewRound(Height::new(1), Round::new(1))),
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(1),
                step: Step::NewRound,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        TestStep {
            desc: "Start round 1, we are not the proposer",
            input_event: Some(Event::NewRound(Height::new(1), Round::new(1))),
            expected_output: Some(Message::ScheduleTimeout(Timeout::propose(Round::new(1)))),
            expected_round: Round::new(1),
            new_state: State {
                height: Height::new(1),
                round: Round::new(1),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
    ];

    let mut event_from_previous_msg = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_event = step
            .input_event
            .unwrap_or_else(|| event_from_previous_msg.unwrap());

        let output = block_on(driver.execute(execute_event)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output message");

        assert_eq!(driver.round_state, step.new_state, "new state");

        event_from_previous_msg = output.and_then(msg_to_event);
    }
}

// No value to propose
#[test]
fn driver_steps_no_value_to_propose() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (my_sk, my_addr) = (sk1, v1.address);
    let ctx = TestContext::new(my_sk.clone());

    // We are the proposer
    let sel = FixedProposer::new(v1.address);
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let output = block_on(driver.execute(Event::NewRound(Height::new(1), Round::new(0))))
        .expect("execute succeeded");

    assert_eq!(
        output,
        Some(Message::GetValueAndScheduleTimeout(
            Round::new(0),
            Timeout::propose(Round::new(0))
        ))
    );
}

#[test]
fn driver_steps_proposer_not_found() {
    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([1, 2, 3]);

    let (my_sk, my_addr) = (sk2, v2.address);

    let ctx = TestContext::new(my_sk.clone());

    // Proposer is v1, which is not in the validator set
    let sel = FixedProposer::new(v1.address);
    let vs = ValidatorSet::new(vec![v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let output = block_on(driver.execute(Event::NewRound(Height::new(1), Round::new(0))));
    assert_eq!(output, Err(Error::ProposerNotFound(v1.address)));
}

#[test]
fn driver_steps_validator_not_found() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2), (v3, sk3)] = make_validators([1, 2, 3]);

    let (my_sk, my_addr) = (sk3.clone(), v3.address);
    let ctx = TestContext::new(my_sk.clone());

    // Proposer is v1
    let sel = FixedProposer::new(v1.address);
    // We omit v2 from the validator set
    let vs = ValidatorSet::new(vec![v1.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    // Start new height
    block_on(driver.execute(Event::NewRound(Height::new(1), Round::new(0))))
        .expect("execute succeeded");

    // v2 prevotes for some proposal, we cannot find it in the validator set => error
    let output = block_on(driver.execute(Event::Vote(
        Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), v2.address).signed(&sk2),
    )));

    assert_eq!(output, Err(Error::ValidatorNotFound(v2.address)));
}

#[test]
fn driver_steps_invalid_signature() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, _sk2), (v3, sk3)] = make_validators([1, 2, 3]);

    let (my_sk, my_addr) = (sk3.clone(), v3.address);
    let ctx = TestContext::new(my_sk.clone());

    let sel = FixedProposer::new(v1.address);
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    // Start new round
    block_on(driver.execute(Event::NewRound(Height::new(1), Round::new(0))))
        .expect("execute succeeded");

    // v2 prevotes for some proposal, with an invalid signature,
    // ie. signed by v1 instead of v2, just a way of forging an invalid signature
    let output = block_on(driver.execute(Event::Vote(
        Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), v2.address).signed(&sk1),
    )));

    assert!(matches!(output, Err(Error::InvalidVoteSignature(_, _))));
}

#[test]
fn driver_steps_skip_round_skip_threshold() {
    let value = Value::new(9999);

    let sel = RotateProposer;

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([1, 1, 1]);

    // Proposer is v1, so we, v3, are not the proposer
    let (my_sk, my_addr) = (sk3, v3.address);

    let ctx = TestContext::new(my_sk.clone());
    let height = Height::new(1);

    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);
    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let steps = vec![
        // Start round 0, we, v3, are not the proposer
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer",
            input_event: Some(Event::NewRound(height, Round::new(0))),
            expected_output: Some(Message::ScheduleTimeout(Timeout::propose(Round::new(0)))),
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // Receive a propose timeout, prevote for nil (from v3)
        TestStep {
            desc: "Receive a propose timeout, prevote for nil (v3)",
            input_event: Some(Event::TimeoutElapsed(Timeout::propose(Round::new(0)))),
            expected_output: Some(Message::Vote(
                Vote::new_prevote(height, Round::new(0), None, my_addr).signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // Receive our own prevote v3
        TestStep {
            desc: "Receive our own prevote v3",
            input_event: None,
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v1 prevotes for its own proposal
        TestStep {
            desc: "v1 prevotes for its own proposal in round 1",
            input_event: Some(Event::Vote(
                Vote::new_prevote(height, Round::new(1), Some(value.id()), v1.address).signed(&sk1),
            )),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v2 prevotes for v1 proposal in round 1, expected output is to move to next round
        TestStep {
            desc: "v2 prevotes for v1 proposal, we get +1/3 messages from future round",
            input_event: Some(Event::Vote(
                Vote::new_prevote(height, Round::new(1), Some(value.id()), v2.address).signed(&sk2),
            )),
            expected_output: Some(Message::NewRound(height, Round::new(1))),
            expected_round: Round::new(1),
            new_state: State {
                height,
                round: Round::new(1),
                step: Step::NewRound,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
    ];

    let mut event_from_previous_msg = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_event = step
            .input_event
            .unwrap_or_else(|| event_from_previous_msg.unwrap());

        let output = block_on(driver.execute(execute_event)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output message");

        assert_eq!(driver.round(), step.expected_round, "expected round");
        assert_eq!(driver.round_state, step.new_state, "new state");

        event_from_previous_msg = output.and_then(msg_to_event);
    }
}

#[test]
fn driver_steps_skip_round_quorum_threshold() {
    let value = Value::new(9999);

    let sel = RotateProposer;

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([1, 2, 1]);

    // Proposer is v1, so we, v3, are not the proposer
    let (my_sk, my_addr) = (sk3, v3.address);

    let ctx = TestContext::new(my_sk.clone());
    let height = Height::new(1);

    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);
    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let steps = vec![
        // Start round 0, we, v3, are not the proposer
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer",
            input_event: Some(Event::NewRound(height, Round::new(0))),
            expected_output: Some(Message::ScheduleTimeout(Timeout::propose(Round::new(0)))),
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Propose,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // Receive a propose timeout, prevote for nil (from v3)
        TestStep {
            desc: "Receive a propose timeout, prevote for nil (v3)",
            input_event: Some(Event::TimeoutElapsed(Timeout::propose(Round::new(0)))),
            expected_output: Some(Message::Vote(
                Vote::new_prevote(height, Round::new(0), None, my_addr).signed(&my_sk),
            )),
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // Receive our own prevote v3
        TestStep {
            desc: "Receive our own prevote v3",
            input_event: None,
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v1 prevotes for its own proposal
        TestStep {
            desc: "v1 prevotes for its own proposal in round 1",
            input_event: Some(Event::Vote(
                Vote::new_prevote(height, Round::new(1), Some(value.id()), v1.address).signed(&sk1),
            )),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
        // v2 prevotes for v1 proposal in round 1, expected output is to move to next round
        TestStep {
            desc: "v2 prevotes for v1 proposal, we get +1/3 messages from future round",
            input_event: Some(Event::Vote(
                Vote::new_prevote(height, Round::new(1), Some(value.id()), v2.address).signed(&sk2),
            )),
            expected_output: Some(Message::NewRound(height, Round::new(1))),
            expected_round: Round::new(1),
            new_state: State {
                height,
                round: Round::new(1),
                step: Step::NewRound,
                proposal: None,
                locked: None,
                valid: None,
            },
        },
    ];

    let mut event_from_previous_msg = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let execute_event = step
            .input_event
            .unwrap_or_else(|| event_from_previous_msg.unwrap());

        let output = block_on(driver.execute(execute_event)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output message");

        assert_eq!(driver.round(), step.expected_round, "expected round");

        assert_eq!(driver.round_state, step.new_state, "new state");

        event_from_previous_msg = output.and_then(msg_to_event);
    }
}
