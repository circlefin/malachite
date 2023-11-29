use futures::executor::block_on;

use malachite_common::Round;
use malachite_driver::{Driver, Input, Output, Validity};
use malachite_round::state::State;

use malachite_test::{Height, Proposal, TestContext, ValidatorSet, Value};

use malachite_test::utils::*;

// TODO - move all below to utils?
struct TestStep {
    desc: &'static str,
    input: Input<TestContext>,
    expected_output: Option<Output<TestContext>>,
    expected_round: Round,
    new_state: State<TestContext>,
}

pub fn output_to_input(output: Output<TestContext>) -> Option<Input<TestContext>> {
    match output {
        Output::NewRound(height, round) => Some(Input::NewRound(height, round)),
        // Let's consider our own proposal to always be valid
        Output::Propose(p) => Some(Input::Proposal(p, Validity::Valid)),
        Output::Vote(v) => Some(Input::Vote(v)),
        Output::Decide(_, _) => None,
        Output::ScheduleTimeout(_) => None,
        Output::GetValueAndScheduleTimeout(_, _) => None,
    }
}

// Arrive at L49 with commits from current rounds, no locked value, no valid value
//
// Ev:             NewRound                    <quorum>                       Proposal
// State: NewRound -------------------> Propose --------------------> Propose -------> Commit
// Msg:            start_propose_timer          start_precommit_timer         decide
// Alg:            L21                          L48                           L49
//
// v1=2, v2=3, v3=2, we are v3
//
// L21 - v3 is not proposer starts propose timer (step propose)
// L46 - v3 gets +2/3 precommits (from v1 and v2), starts precommit timer (step propose)
// L49 - v3 receives proposal and has already +2/3 precommit(id(v), round=0) (step decided)
#[test]
fn driver_steps_decide_current_with_no_locked_no_valid() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = RotateProposer;
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v2, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0)),
            expected_output: start_propose_timer_output(Round::new(0)),
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 precommits a proposal",
            input: precommit_input(Round::new(0), value, &v1.address, &sk1),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits for same proposal, we get +2/3 precommit, start precommit timer",
            input: precommit_input(Round::new(0), value, &v2.address, &sk2),
            expected_output: start_precommit_timer_output(Round::new(0)),
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal",
            input: proposal_input(Round::new(0), value, Round::Nil, Validity::Valid),
            expected_output: decide_output(Round::new(0), value),
            expected_round: Round::new(0),
            new_state: decided_state(
                Round::new(0),
                // Proposal::new(Height::new(1), Round::new(1), value, Round::new(0)), <--- TODO - should be this?
                value,
            ),
        },
    ];

    run_steps(&mut driver, steps)
}

// Arrive at L49 with commits from previous rounds, no locked value, no valid value
//
// Ev:             NewRound(0)           Timeout(propose)          <polka>               Timeout(prevote)
// State: NewRound ------------> Propose ----------------> Prevote ------------> Prevote ---------------> Precommit -->
// Msg:            propose_timer         Prevote(nil)              prevote_timer         Precommit(nil)
// Alg:            L21                   L57                       L34                   L61
//
// Ev:                  <quorum>              Timeout(precommit)         NewRound(1)          Proposal+<quorum>
// State: --> Precommit ----------> Precommit ---------------> NewRound -----------> Propose -----------------> Decided
// Msg:                 precommit_timer       new_round(1)              propose_timer         decide
// Alg:                 L46                   L65                       L21                   L49
//
// v1=2, v2=3, v3=2, we are v3
// L21 - v3 is not proposer starts propose timer (step propose)
// L57 - v3 receives timeout propose, prevote for nil (step prevote)
// L34 - v3 gets +2/3 prevotes (from v1 and v2), starts prevote timer (step prevote)
// L61 - v3 receives timeout prevote, precommit nil (step precommit)
// L46 - v3 gets +2/3 precommits (from v1 and v2), starts precommit timer (step precommit)
// L65 - v3 receives timeout precommit, starts new round (step new_round)
// L21 - v3 receives new round, is not the proposer, starts propose timer
// L49 - v3 receives proposal(v, round=0) and has already +2/3 precommit(id(v), round=0) (step decided)
#[test]
fn driver_steps_decide_previous_with_no_locked_no_valid() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = RotateProposer;
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v2, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0)),
            expected_output: start_propose_timer_output(Round::new(0)),
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propopse, prevote for nil (v2)",
            input: timeout_propose_input(Round::new(0)),
            expected_output: prevote_nil_output(Round::new(0), &my_addr, &my_sk),
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes a proposal",
            input: prevote_input(&v1.address, &sk1),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for same proposal, we get +2/3 prevotes, start prevote timer",
            input: prevote_input(&v2.address, &sk2),
            expected_output: start_prevote_timer_output(Round::new(0)),
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 precommits a proposal",
            input: precommit_input(Round::new(0), value, &v1.address, &sk1),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits for same proposal, we get +2/3 precommit, start precommit timer",
            input: precommit_input(Round::new(0), value, &v2.address, &sk2),
            expected_output: start_precommit_timer_output(Round::new(0)),
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout precommit, start new round",
            input: timeout_precommit_input(Round::new(0)),
            expected_output: new_round_output(Round::new(1)),
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Receive proposal",
            input: proposal_input(Round::new(0), value, Round::Nil, Validity::Valid),
            expected_output: decide_output(Round::new(0), value),
            expected_round: Round::new(1),
            new_state: decided_state(
                Round::new(1),
                // Proposal::new(Height::new(1), Round::new(1), value, Round::new(0)), <--- TODO - should be this
                value,
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

// Arrive at L36 in round 0, with step prevote and then L28 in round 1, with locked value v.
//
// Ev:             NewRound(0)           Proposal           <polka>                <honest precommit(round=1)>
// State: NewRound ------------> Propose ---------> Prevote -----------> Precommit ---------------------------> NewRound -->
// Msg:            propose_timer         prevote(v)         precommit(v)           new_round(1)
// Alg:            L21                   L24                L37                    L56
//
// Ev:                  NewRound(1)             Proposal(+polka)
// State: --> NewRound ---------------> Propose ------------------> Prevote
// Msg:                 propose(v, pol)         prevote(v,round=1)
// Alg:                 L16, L19                L28-L30
//
// v1=2, v2=2, v3=3, we are v2
// Trying to arrive at L36 with step prevote and then L28
// L21 - v2 is not proposer starts timeout propose (step propose)
// L24 - v2 receives proposal(v) from v1, prevotes for v (step prevote)
// L37 - v1 and v3 prevote for v, v2 gets +2/3 prevotes, locked_value=v, valid_value=v, sends precommit(v) (step precommit)
// L56 - v2 receives a precommit(id(v), round=1) from v3, starts new round (step new_round)
//   Note - this doesn't seem correct v2 behaviour (??)
// L16, L19 - v2 is the proposer and has both a locked and valid value from round 0, propose(round=1, value=v, valid_round=0) (step propose)
// L28 - v2 receives its proposal and has 2f+1 prevotes from round 0 and:
//   L29 - locked_round(0) <= valid_round(0) and valid_round(0) < round(1)
//     L30 - v2 sends prevote(id(v), round=1) (step prevote)
#[test]
fn driver_steps_polka_previous_with_locked() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([2, 2, 3]);
    let (my_sk, my_addr) = (sk2, v2.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = RotateProposer;
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v2, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0)),
            expected_output: start_propose_timer_output(Round::new(0)),
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "receive a proposal from v1 - L22 send prevote",
            input: proposal_input(Round::new(0), value, Round::Nil, Validity::Valid),
            expected_output: prevote_output(Round::new(0), &my_addr, &my_sk),
            expected_round: Round::new(0),
            new_state: prevote_state_with_proposal(
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "v3 prevotes the proposal",
            input: prevote_input(&v3.address, &sk3),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: prevote_state_with_proposal(
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "v1 prevotes for same proposal, we get +2/3 prevotes, precommit",
            input: prevote_input(&v1.address, &sk1),
            expected_output: precommit_output(Round::new(0), value, &my_addr, &my_sk),
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v3",
            input: precommit_input(Round::new(1), Value::new(8888), &v3.address, &sk3),
            expected_output: new_round_output(Round::new(1)),
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "start round 1, we are proposer with a valid value, propose it",
            input: new_round_input(Round::new(1)),
            expected_output: proposal_output(Round::new(1), value, Round::new(0)),
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "Receive our own proposal",
            input: proposal_input(Round::new(1), value, Round::new(0), Validity::Valid),
            expected_output: prevote_output(Round::new(1), &my_addr, &my_sk),
            expected_round: Round::new(1),
            new_state: prevote_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(1), value, Round::new(0)),
            ),
        },
    ];

    run_steps(&mut driver, steps)
}

#[test]
fn driver_steps_polka_previous_invalid_proposal_with_locked() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([2, 2, 3]);
    let (my_sk, my_addr) = (sk2, v2.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = RotateProposer;
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v2, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0)),
            expected_output: start_propose_timer_output(Round::new(0)),
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "receive a proposal from v1 - L22 send prevote",
            input: proposal_input(Round::new(0), value, Round::Nil, Validity::Valid),
            expected_output: prevote_output(Round::new(0), &my_addr, &my_sk),
            expected_round: Round::new(0),
            new_state: prevote_state_with_proposal(
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "v3 prevotes the proposal",
            input: prevote_input(&v3.address, &sk3),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: prevote_state_with_proposal(
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "v1 prevotes for same proposal, we get +2/3 prevotes, precommit",
            input: prevote_input(&v1.address, &sk1),
            expected_output: precommit_output(Round::new(0), value, &my_addr, &my_sk),
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v3",
            input: precommit_input(Round::new(1), Value::new(8888), &v3.address, &sk3),
            expected_output: new_round_output(Round::new(1)),
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "start round 1, we are proposer with a valid value, propose it",
            input: new_round_input(Round::new(1)),
            expected_output: proposal_output(Round::new(1), value, Round::new(0)),
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "Receive our own proposal",
            input: proposal_input(Round::new(1), value, Round::new(0), Validity::Invalid),
            expected_output: prevote_nil_output(Round::new(1), &my_addr, &my_sk),
            expected_round: Round::new(1),
            new_state: prevote_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

// Arrive at L36 in round 0, with step precommit and then L28 in round 1 with no locked value.
//
// Ev:             NewRound(0)           Timeout(propose)         <polka>              Timeout(prevote)
// State: NewRound ------------> Propose ---------------> Prevote -----------> Prevote -----------------> Precommit -->
// Msg:            propose_timer         Prevote(nil)             prevote_timer        Precommit(nil)
// Alg:            L21                   L59                      L34                  L63
//
// Ev:                  Proposal(v)                <honest precommit(round=1)>
// State: --> Precommit ---------------> Precommit ---------------------------> NewRound
// Msg:                 none                       new_round(1)
// Alg:                 L42, L43                   L56
//
// Ev:                  NewRound(1)             Proposal(+polka)
// State: --> NewRound ---------------> Propose -------------------> Prevote
// Msg:                 propose(v, pol)         prevote(nil,round=1)
// Alg:                 L16, L19                L28, L32 (not locked on v)
//
// v1=2, v2=2, v3=3, we are v2
// Trying to be at L36 with step precommit
// L21 - v2 is not proposer starts timeout propose (step propose)
// L59 - v2 receives timeout propose, prevotes for nil (step prevote)
// L35 - v1 and v3 prevote for some proposal(v), v2 gets +2/3 prevotes, starts timeout prevote (step prevote)
// L63 - v2 receives timeout prevote, prevotes for nil (step precommit)
// L36 - v2 receives the proposal(v) from v1, sets valid = v (L42, L43) but does NOT lock (L37-L41 not executed) (step precommit)
// L56 - v2 receives a prevote(id(v), round=1) from v3, starts new round (step new_round)
//   Note - this doesn't seem correct v2 behaviour
// L16, L19 - v2 is the proposer and has a valid value from round 0, propose(round=1, value=v, valid_round=0) (step propose)
// L28 - v2 receives its proposal and has 2f+1 prevotes from round 0 and:
//   L29 - locked_round(-1) < valid_round(0) and valid_round(0) < round(1) BUT locked_value is nil
//     L32 - v2 sends prevote(nil, round=1) (step prevote)
#[test]
fn driver_steps_polka_previous_with_no_locked() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, sk2), (v3, sk3)] = make_validators([2, 2, 3]);
    let (my_sk, my_addr) = (sk2, v2.address);

    let ctx = TestContext::new(my_sk.clone());
    let sel = RotateProposer;
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, sel, vs, my_addr);

    let steps = vec![
        TestStep {
            desc: "Start round 0, we v2 are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0)),
            expected_output: start_propose_timer_output(Round::new(0)),
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propopse, prevote for nil (v2)",
            input: timeout_propose_input(Round::new(0)),
            expected_output: prevote_nil_output(Round::new(0), &my_addr, &my_sk),
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v3 prevotes for some proposal",
            input: prevote_input(&v3.address, &sk3),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes for same proposal, we get +2/3 prevotes, start timeout prevote",
            input: prevote_input(&v1.address, &sk1),
            expected_output: start_prevote_timer_output(Round::new(0)),
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "timeout prevote, prevote for nil (v2)",
            input: timeout_prevote_input(Round::new(0)),
            expected_output: precommit_nil_output(&my_addr, &my_sk),
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "receive  a proposal - L36, we don't lock, we set valid",
            input: proposal_input(Round::new(0), value, Round::Nil, Validity::Valid),
            expected_output: None,
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_valid(
                Round::new(0),
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v3",
            input: prevote_input_at(Round::new(1), &v3.address, &sk3),
            expected_output: new_round_output(Round::new(1)),
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "start round 1, we are proposer with a valid value from round 0, propose it",
            input: new_round_input(Round::new(1)),
            expected_output: proposal_output(Round::new(1), value, Round::new(0)),
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_valid(
                Round::new(1),
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "Receive our own proposal, prevote nil as we are not locked on the value",
            input: proposal_input(Round::new(1), value, Round::new(0), Validity::Valid),
            expected_output: prevote_nil_output(Round::new(1), &my_addr, &my_sk),
            expected_round: Round::new(1),
            new_state: prevote_state_with_proposal_and_valid(
                Round::new(1),
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

fn run_steps(driver: &mut Driver<TestContext>, steps: Vec<TestStep>) {
    for step in steps {
        println!("Step: {}", step.desc);

        let output = block_on(driver.execute(step.input)).expect("execute succeeded");
        assert_eq!(output, step.expected_output, "expected output");

        assert_eq!(
            driver.round_state.round, step.expected_round,
            "expected round"
        );

        assert_eq!(driver.round_state, step.new_state, "expected state");
    }
}
