use malachite_common::{Round, Validity};
use malachite_driver::{Driver, Input, Output};
use malachite_round::state::State;

use malachite_test::utils::driver::*;
use malachite_test::utils::validators::make_validators;
use malachite_test::{Height, Proposal, TestContext, ValidatorSet, Value};

// The following tests are performed:
// - L49 with commits from current rounds, no locked value, no valid value:
//    `driver_steps_decide_current_with_no_locked_no_valid()`
//
// - L49 with commits from previous rounds, no locked value, no valid value:
//    `driver_steps_decide_previous_with_no_locked_no_valid()`
//
// - L49 with commits from previous round, with locked and valid values
//    `driver_steps_decide_previous_with_locked_and_valid()`
//
// - L28 in round 1, via L36 in round 0, with locked invalid value v.
//     `driver_steps_polka_previous_invalid_proposal()`'
//
// - L23 in round 1, via L36 in round 0, with lockedValue != v.
//     `driver_steps_polka_previous_new_proposal()`
//
// - L28 in round 1 with no locked value, via L36 in round 0 with step precommit.
//     `driver_steps_polka_previous_with_no_locked()`
//
// - L28 in round 1 with locked value, via L36 in round 0 with step prevote.
//      `driver_steps_polka_previous_with_locked()
//
// - L44 with previously received polkaNil and entering prevote (due to timeoutPropose)
//      `driver_steps_polka_nil_and_timeout_propose()`
//
// - L36 with previoustly received polkaValue and proposal, and entering prevote (due to received proposal)
//      `driver_steps_polka_value_then_proposal()`
//
// - L34 with previously received polkaAny and entering prevote (due to received proposal)
//      `driver_steps_polka_any_then_proposal_other()`

struct TestStep {
    desc: &'static str,
    input: Input<TestContext>,
    expected_outputs: Vec<Output<TestContext>>,
    expected_round: Round,
    new_state: State<TestContext>,
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

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v1.address);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 precommits a proposal",
            input: precommit_input(Round::new(0), value, &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits for same proposal, we get +2/3 precommit, start precommit timer",
            input: precommit_input(Round::new(0), value, &v2.address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal",
            input: proposal_input(
                Round::new(0),
                value,
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![decide_output(Round::new(0), proposal)],
            expected_round: Round::new(0),
            new_state: decided_state(Round::new(0), value),
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
// Ev:              <quorum>              Timeout(precommit)         NewRound(1)          Proposal+<quorum>
// State: Precommit ----------> Precommit ---------------> NewRound -----------> Propose -----------------> Decided
// Msg:             precommit_timer       new_round(1)              propose_timer         decide
// Alg:             L46                   L65                       L21                   L49
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

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let proposal = Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v1.address);

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propopse, prevote for nil (v3)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes a proposal",
            input: prevote_input(value, &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for same proposal, we get +2/3 prevotes, start prevote timer",
            input: prevote_input(value, &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 precommits a proposal",
            input: precommit_input(Round::new(0), value, &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits for same proposal, we get +2/3 precommit, start precommit timer",
            input: precommit_input(Round::new(0), value, &v2.address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout precommit, start new round",
            input: timeout_precommit_input(Round::new(0)),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Receive proposal",
            input: proposal_input(
                Round::new(0),
                value,
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![decide_output(Round::new(1), proposal)],
            expected_round: Round::new(1),
            new_state: decided_state(Round::new(1), value),
        },
    ];

    run_steps(&mut driver, steps);
}

// Arrive at L49 with commits from previous round, with locked and valid values
//
// Ev:             NewRound(0)           Timeout(propose)          <polka>               Proposal
// State: NewRound ------------> Propose ----------------> Prevote ------------> Prevote ---------------> Precommit -->
// Msg:            propose_timer         Prevote(nil)              prevote_timer         Precommit(value)
// Alg:            L21                   L57                       L34                   L40
//
// Ev:              <honest precommit(round=1)>            NewRound(1)          <quorum precommit>
// State: Precommit ---------------------------> NewRound -----------> Propose --------------------> Commit
// Msg:             new_round(1)                 propose_timer                  decide(v, round=1)
// Alg:             L56                          L21                            L49-L54
//
// v1=2, v2=3, v3=2, we are v3
// L21 - start round 0, we, v3, are not the proposer, start timeout propose (step propose)
// L57 - timeout propose, prevote for nil (v2) (step prevote)
// L34 - v1 and v2 prevote for same proposal, we get +2/3 prevotes, start prevote timer (step prevote)
// L37-L43 - v3 receives the proposal, sets locked and value (step precommit)
// L55 - v2 precommits for value in round 1, i.e. v3 receives f+1 vote for round 1 from v2 (step new_round)
// L11 - v3 starts new round, not proposer, start timeout propose (step propose)
// L49 - v3 gets +2/3 precommits (from v1 and v2) for round 0, it has the proposal, decide
//
#[test]
fn driver_steps_decide_previous_with_locked_and_valid() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let proposal = Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v1.address);

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propopse, prevote for nil (v3)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes a proposal",
            input: prevote_input(value, &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for same proposal, we get +2/3 prevotes, start prevote timer",
            input: prevote_input(value, &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal, L37-L43",
            input: proposal_input(
                Round::new(0),
                value,
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![precommit_output(
                Round::new(0),
                Value::new(9999),
                &v3.address,
            )],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "v2 precommits for value in round 1, i.e. f+1 vote for round 1 from v2",
            input: precommit_input(Round::new(1), Value::new(9999), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "v1 precommits for round 0 and same proposal",
            input: precommit_input(Round::new(0), value, &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "v2 precommits for round 0 and same proposal, we get +2/3 precommit, decide",
            input: precommit_input(Round::new(0), value, &v2.address),
            expected_outputs: vec![decide_output(Round::new(1), proposal.clone())],
            expected_round: Round::new(1),
            new_state: decided_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal.clone(),
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
// Ev:             NewRound(1)             Proposal(+polka)
// State: NewRound --------------> Propose -----------------> Prevote
// Msg:            propose(v, pol)         prevote(v,round=1)
// Alg:            L16, L19                L28-L30
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

    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([2, 2, 3]);
    let (my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v2, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "receive a proposal from v1 - L22 send prevote",
            input: proposal_input(
                Round::new(0),
                value,
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value, &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(
                Round::new(0),
                // Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "v3 prevotes the proposal",
            input: prevote_input(value, &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(
                Round::new(0),
                // Proposal::new(Height::new(1), Round::new(0), value, Round::Nil),
            ),
        },
        TestStep {
            desc: "v1 prevotes for same proposal, we get +2/3 prevotes, precommit",
            input: prevote_input(value, &v1.address),
            expected_outputs: vec![precommit_output(Round::new(0), value, &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v1.address),
            ),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v3",
            input: precommit_input(Round::new(1), Value::new(8888), &v3.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v1.address),
            ),
        },
        TestStep {
            desc: "start round 1, we are proposer with a valid value, propose it",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![proposal_output(
                Round::new(1),
                value,
                Round::new(0),
                v2.address,
            )],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v2.address),
            ),
        },
        TestStep {
            desc: "Receive our own proposal",
            input: proposal_input(
                Round::new(1),
                value,
                Round::new(0),
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(1), value, &v2.address)],
            expected_round: Round::new(1),
            new_state: prevote_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value,
                    Round::new(0),
                    v2.address,
                ),
            ),
        },
    ];

    run_steps(&mut driver, steps)
}

// Arrive at L36 in round 0, with step precommit and then L28 in round 1 with invalid value.
//
// Ev:             NewRound(0)           Timeout(propose)        <polka>                <honest precommit(round=1)>
// State: NewRound ------------> Propose --------------> Prevote -------------> Prevote ---------------------------> NewRound -->
// Msg:            propose_timer         prevote(nil)            prevote_timer          new_round(1)
// Alg:            L21                   L59                     L35                    L56
//
// Ev:             NewRound(1)              InvalidProposal(round=0)
// State: NewRound ---------------> Propose -----------------------> Prevote
// Msg:            propose_timer            prevote(nil,round=1)
// Alg:            L21                      L28-L32
//
// v1=2, v2=3, v3=2, we are v3
// L21 - v3 is not proposer starts timeout propose (step propose)
// L57 - propose timeout, prevote nil (step prevote)
// L37 - v1 and v2 prevote for v, v3 gets +2/3 prevotes, start timeout prevote (step prevote)
// L55 - Receive f+1 vote for round 1 from v2, start new round (step new_round)
// L21 - v3 is not proposer starts timeout propose (step propose)
// L28 - v3 receives invalid proposal and has 2f+1 prevotes from round 0 and:
//   L29 - with invalid proposal
//     L32 - v2 sends prevote(nil, round=1) (step prevote)
#[test]
fn driver_steps_polka_previous_invalid_proposal() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3, v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "timeout propopse, prevote for nil (v3)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes a proposal",
            input: prevote_input(value, &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for same proposal, we get +2/3 prevotes, start prevote timer",
            input: prevote_input(value, &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v2",
            input: prevote_input_at(Round::new(1), value, &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "receive an invalid proposal for POL round 0",
            input: proposal_input(
                Round::new(1),
                value,
                Round::new(0),
                Validity::Invalid,
                v1.address,
            ),
            expected_outputs: vec![prevote_nil_output(Round::new(1), &my_addr)],
            expected_round: Round::new(1),
            new_state: prevote_state(Round::new(1)),
        },
    ];

    run_steps(&mut driver, steps);
}

// Arrive at L36 in round 0, with step precommit and then L23 in round 1 with lockedValue != v.
//
// Ev:             NewRound(0)           Proposal           <polka>                <honest precommit(round=1)>
// State: NewRound ------------> Propose ---------> Prevote -----------> Precommit ---------------------------> NewRound -->
// Msg:            propose_timer         prevote(v)         precommit(v)           new_round(1)
// Alg:            L21                   L24                L37                    L56
//
// Ev:             NewRound(1)              Proposal(other_value, pol_round=0)
// State: NewRound ---------------> Propose ----------------------------------> Prevote
// Msg:            propose_timer            prevote(nil,round=1)
// Alg:            L21                      L26
//
// v1=2, v2=3, v3=2, we are v3
// L21 - v3 is not proposer starts timeout propose (step propose)
// L24 - v3 receives proposal, prevotes for value  (step prevote)
// L37 - v1 and v2 prevote for v, v3 gets +2/3 prevotes (step precommit)
// L56 - Receive f+1 vote for round 1 from v2, start new round (step new_round)
// L21 - v3 is not proposer starts timeout propose (step propose)
// L22 - v3 receives proposal for a different value and no POL round:
//   L23 - valid(v) and lockedValue != v
//     L26 - v2 sends prevote(nil, round=1) (step prevote)
#[test]
fn driver_steps_polka_previous_new_proposal() {
    let value = Value::new(9999);
    let other_value = Value::new(8888);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3, v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "receive a valid proposal for round 0",
            input: proposal_input(
                Round::new(0),
                value,
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value, &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes the proposal",
            input: prevote_input(value, &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for same proposal, we get +2/3 prevotes, precommit",
            input: prevote_input(value, &v2.address),
            expected_outputs: vec![precommit_output(Round::new(0), value, &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v1.address),
            ),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v2",
            input: prevote_input_at(Round::new(1), value, &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v1.address),
            ),
        },
        TestStep {
            desc: "start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v1.address),
            ),
        },
        TestStep {
            desc: "receive a valid proposal for round 1 with different value",
            input: proposal_input(
                Round::new(1),
                other_value,
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_nil_output(Round::new(1), &my_addr)],
            expected_round: Round::new(1),
            new_state: prevote_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value,
                    Round::new(0),
                    v1.address,
                ),
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
// Ev:              Proposal(v)           <honest precommit(round=1)>
// State: Precommit ----------> Precommit --------------------------> NewRound -->
// Msg:             none                  new_round(1)
// Alg:             L42, L43              L56
//
// Ev:             NewRound(1)             Proposal(+polka)
// State: NewRound --------------> Propose -------------------------> Prevote
// Msg:            propose(v, pol)         prevote(nil,round=1)
// Alg:            L16, L19                L28, L32 (not locked on v)
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

    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([2, 2, 3]);
    let (my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v2, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propose, prevote for nil (v2)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v3 prevotes for some proposal",
            input: prevote_input(value, &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes for same proposal, we get +2/3 prevotes, start timeout prevote",
            input: prevote_input(value, &v1.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "timeout prevote, prevote for nil (v2)",
            input: timeout_prevote_input(Round::new(0)),
            expected_outputs: vec![precommit_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "receive  a proposal - L36, we don't lock, we set valid",
            input: proposal_input(
                Round::new(0),
                value,
                Round::Nil,
                Validity::Valid,
                v3.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_valid(
                Round::new(0),
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v3.address),
            ),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v3",
            input: prevote_input_at(Round::new(1), value, &v3.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v3.address),
            ),
        },
        TestStep {
            desc: "start round 1, we are proposer with a valid value from round 0, propose it",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![proposal_output(
                Round::new(1),
                value,
                Round::new(0),
                v2.address,
            )],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_valid(
                Round::new(1),
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v2.address),
            ),
        },
        TestStep {
            desc: "Receive our own proposal, prevote value even if we are not locked on it (L29 with lockedRoundp == nil < vr)",
            input: proposal_input(
                Round::new(1),
                value,
                Round::new(0),
                Validity::Valid,
                v2.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(1), value, &my_addr)],
            expected_round: Round::new(1),
            new_state: prevote_state_with_proposal_and_valid(
                Round::new(1),
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v2.address),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

// Arrive at L44 with previously received polkaNil and entering prevote (due to timeoutPropose)
//
// Ev:             NewRound(0)          <polkaNil>         Timeout(propose)         + replay <polkaNil>
// State: NewRound ------------> Propose --------> Propose ---------------> Prevote -------------------> Precommit
// Msg:            propose_timer         None              prevote_nil              precommit_nil
// Alg:            L21                                     L34                      L44
//
//
// v1=2, v2=3, v3=2, we are v3
// L21 - v3 is not proposer starts propose timer (step propose)
// L34 - v3 gets +2/3 prevotes for nil (from v1 and v2), event ignored (step propose)
// L57 - v3 receives timeout propose, prevotes for nil (step prevote)
// L44 - polkaNil is replayed and v3 precommits for nil (step precommit)
#[test]
fn driver_steps_polka_nil_and_timeout_propose() {
    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes nil",
            input: prevote_nil_input(&v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for for nil, we get polkaNil, but we are in Propose step",
            input: prevote_nil_input(&v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propose, prevote for nil then precommit for nil",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![
                prevote_nil_output(Round::new(0), &my_addr),
                precommit_nil_output(Round::new(0), &my_addr),
            ],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
    ];

    run_steps(&mut driver, steps);
}

// Arrive at L36 with previoustly received polkaValue and proposal, and entering prevote (due to received proposal)
//
// Ev:             NewRound(0)           <polkaValue>         Proposal           + replay <polkaValue>
// State: NewRound ------------> Propose -----------> Propose ---------> Prevote --------------------> Precommit
// Msg:            propose_timer         None                 prevote(v)         precommit(v)
// Alg:            L21                                        L24                L37, L37-L43
//
//
// v1=2, v2=3, v3=2, we are v3
// L21 - v3 is not proposer starts propose timer (step propose)
// L34 - v3 gets +2/3 prevotes (from v1 and v2), events ignored (step propose)
// L57 - v3 receives proposal, prevotes for value  (step prevote)
// L36 - polka is replayed and v3 precommits for value (step precommit)
#[test]
fn driver_steps_polka_value_then_proposal() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps =
        vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes a proposal",
            input: prevote_input(value, &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for same proposal, we get +2/3 prevotes, but we are in Propose step",
            input: prevote_input(value, &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "receive a proposal from v1 - L22 send prevote",
            input: proposal_input(Round::new(0), value, Round::Nil, Validity::Valid, v1.address),
            expected_outputs: vec![
                prevote_output(Round::new(0),value, &my_addr),
                precommit_output(Round::new(0), value, &my_addr),
            ],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value, Round::Nil, v1.address),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

// Arrive at L34 with previously received polkaAny and entering prevote (due to received proposal)
//
// Ev:             NewRound(0)           <polkaAny(v)>          Proposal(v')         + replay <polkaAny>
// State: NewRound ------------> Propose -------------> Propose -----------> Prevote -------------------------> Prevote
// Msg:            propose_timer         None                   prevote(v)           schedule_timeout(prevote)
// Alg:            L21                                          L24                  L34
//
//
// v1=2, v2=3, v3=2, we are v3
// L21 - v3 is not proposer starts propose timer (step propose)
// L34 - v3 gets +2/3 prevotes for v (from v1 and v2), events ignored (step propose)
// L57 - v3 receives proposal for v', prevotes for v'  (step prevote)
// L34 - polka any is replayed and prevote timer is started (step prevote)
#[test]
fn driver_steps_polka_any_then_proposal_other() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes for nil",
            input: prevote_nil_input(&v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for same proposal, we get polkaAny, but we are in Propose step",
            input: prevote_input(value, &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "receive a proposal from v1 - L22 send prevote, replay polkaAny, start timeout prevote",
            input: proposal_input(Round::new(0), value, Round::Nil, Validity::Valid, v1.address),
            expected_outputs: vec![
                prevote_output(Round::new(0),value, &my_addr),
                start_prevote_timer_output(Round::new(0))
            ],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
fn driver_equivocate_vote() {
    let value1 = Value::new(9999);
    let value2 = Value::new(42);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value1,
        Round::Nil,
        v1.address,
    );

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "start round 0, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "receive a proposal from v1, start timeout prevote",
            input: proposal_input(
                Round::new(0),
                value1,
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value1, &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for proposal",
            input: prevote_input(value1, &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for different value",
            input: prevote_input(value2, &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes for proposal, v3 precommits with proposal and locked and valid",
            input: prevote_input(value1, &v1.address),
            expected_outputs: vec![precommit_output(Round::new(0), value1, &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(Round::new(0), proposal),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
fn driver_equivocate_proposal() {
    let value1 = Value::new(9999);
    let value2 = Value::new(42);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new(my_sk.clone());
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value1,
        Round::Nil,
        v1.address,
    );

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "start round 0, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "receive proposal 1 from v1, start timeout prevote",
            input: proposal_input(
                Round::new(0),
                value1,
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value1, &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "receive proposal 2 from v1",
            input: proposal_input(
                Round::new(0),
                value2,
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes for proposal",
            input: prevote_input(value1, &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes for proposal, v3 precommits with proposal and locked and valid",
            input: prevote_input(value1, &v1.address),
            expected_outputs: vec![precommit_output(Round::new(0), value1, &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

fn run_steps(driver: &mut Driver<TestContext>, steps: Vec<TestStep>) {
    for step in steps {
        println!("Step: {}", step.desc);

        let outputs = driver.process(step.input).expect("execute succeeded");

        assert_eq!(outputs, step.expected_outputs, "expected outputs");
        assert_eq!(driver.round(), step.expected_round, "expected round");
        assert_eq!(driver.round_state(), &step.new_state, "expected state");
    }
}
