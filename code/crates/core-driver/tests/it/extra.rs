use futures::executor::block_on;
use malachitebft_core_state_machine::state::State;
use malachitebft_core_types::{
    CommitCertificate, Context, NilOrVal, Round, ThresholdParams, Validity,
};
use malachitebft_signing::{SigningProvider, SigningProviderExt};

use malachitebft_test::utils::validators::{make_validators, make_validators_seeded};
use malachitebft_test::{Ed25519Provider, Height, Proposal, TestContext, ValidatorSet, Value};

use arc_malachitebft_core_driver::{Driver, Input, Output};

use malachitebft_core_state_machine::state::Step;

use crate::utils::*;

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
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value.clone(),
        Round::Nil,
        v1.address,
    );

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
            input: precommit_input(Round::new(0), value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits the same proposal, we get +2/3 precommit, start precommit timer",
            input: precommit_input(Round::new(0), value.clone(), &v2.address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal",
            input: proposal_input(
                Round::new(0),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![decide_output(Round::new(0), proposal)],
            expected_round: Round::new(0),
            new_state: decided_state(Round::new(0), Round::new(0), value),
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
// L57 - v3 receives timeout propose, prevote nil (step prevote)
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
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value.clone(),
        Round::Nil,
        v1.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propopse, prevote nil (v3)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes a proposal",
            input: prevote_input(value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes same proposal, we get +2/3 prevotes, start prevote timer",
            input: prevote_input(value.clone(), &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 precommits a proposal",
            input: precommit_input(Round::new(0), value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits the same proposal, we get +2/3 precommit, start precommit timer",
            input: precommit_input(Round::new(0), value.clone(), &v2.address),
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
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Receive proposal",
            input: proposal_input(
                Round::new(0),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![decide_output(Round::new(1), proposal)],
            expected_round: Round::new(1),
            new_state: decided_state(Round::new(1), Round::new(0), value),
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
// L57 - timeout propose, prevote nil (v2) (step prevote)
// L34 - v1 and v2 prevote same proposal, we get +2/3 prevotes, start prevote timer (step prevote)
// L37-L43 - v3 receives the proposal, sets locked and value (step precommit)
// L55 - v2 precommits the value in round 1, i.e. v3 receives f+1 vote for round 1 from v2 (step new_round)
// L11 - v3 starts new round, not proposer, start timeout propose (step propose)
// L49 - v3 gets +2/3 precommits (from v1 and v2) for round 0, it has the proposal, decide
//
#[test]
fn driver_steps_decide_previous_with_locked_and_valid() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value.clone(),
        Round::Nil,
        v1.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propopse, prevote nil (v3)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes a proposal",
            input: prevote_input(value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes same proposal, we get +2/3 prevotes, start prevote timer",
            input: prevote_input(value.clone(), &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal, L37-L43",
            input: proposal_input(
                Round::new(0),
                value.clone(),
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
            desc: "v2 precommits the value in round 1, we get f+1 votes for round 1 from v2",
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
            input: precommit_input(Round::new(0), value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "v2 precommits for round 0 and same proposal, we get +2/3 precommit, decide",
            input: precommit_input(Round::new(0), value.clone(), &v2.address),
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
// L24 - v2 receives proposal(v) from v1, prevotes v (step prevote)
// L37 - v1 and v3 prevote v, v2 gets +2/3 prevotes, locked_value=v, valid_value=v, sends precommit(v) (step precommit)
// L56 - v2 receives a precommit(id(v), round=1) from v3, starts new round (step new_round)
//   Note - this doesn't seem correct v3 behaviour (??)
// L16, L19 - v2 is the proposer and has both a locked and valid value from round 0, propose(round=1, value=v, valid_round=0) (step propose)
// L28 - v2 receives its proposal and has 2f+1 prevotes from round 0 and:
//   L29 - locked_round(0) <= valid_round(0) and valid_round(0) < round(1)
//     L30 - v2 sends prevote(id(v), round=1) (step prevote)
#[test]
fn driver_steps_polka_previous_with_locked() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([2, 2, 3]);
    let (_my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Receive a proposal from v1 - L22 send prevote",
            input: proposal_input(
                Round::new(0),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(
                Round::new(0),
                // Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::Nil),
            ),
        },
        TestStep {
            desc: "v3 prevotes the proposal",
            input: prevote_input(value.clone(), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(
                Round::new(0),
                // Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::Nil),
            ),
        },
        TestStep {
            desc: "v1 prevotes same proposal, we get +2/3 prevotes, precommit",
            input: prevote_input(value.clone(), &v1.address),
            expected_outputs: vec![precommit_output(Round::new(0), value.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                Proposal::new(
                    Height::new(1),
                    Round::new(0),
                    value.clone(),
                    Round::Nil,
                    v1.address,
                ),
            ),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v3",
            input: precommit_input(Round::new(1), Value::new(8888), &v3.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(0),
                    value.clone(),
                    Round::Nil,
                    v1.address,
                ),
            ),
        },
        TestStep {
            desc: "Start round 1, we are proposer with a valid value, propose it",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![proposal_output(
                Round::new(1),
                value.clone(),
                Round::new(0),
                v2.address,
            )],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(0),
                    value.clone(),
                    Round::Nil,
                    v2.address,
                ),
            ),
        },
        TestStep {
            desc: "Receive our own proposal",
            input: proposal_input(
                Round::new(1),
                value.clone(),
                Round::new(0),
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(1), value.clone(), &v2.address)],
            expected_round: Round::new(1),
            new_state: prevote_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value.clone(),
                    Round::new(0),
                    v2.address,
                ),
            ),
        },
    ];

    run_steps(&mut driver, steps)
}

#[test]
fn driver_steps_polka_previous_not_locked_vote_last_l30() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([2, 2, 3]);
    let (_my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Receive a proposal from v1 - L22 send prevote",
            input: proposal_input(
                Round::new(0),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes the proposal",
            input: prevote_input(value.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v3 proposes with valid round at round 1",
            input: proposal_input(
                Round::new(1),
                value.clone(),
                Round::new(0),
                Validity::Valid,
                v3.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v3",
            input: precommit_input(Round::new(1), Value::new(8888), &v3.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1",
            input: new_round_input(Round::new(1), v3.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "v3 prevotes the proposal at round 0",
            input: prevote_input(value.clone(), &v3.address),
            expected_outputs: vec![prevote_output(Round::new(1), value.clone(), &my_addr)],
            expected_round: Round::new(1),
            new_state: prevote_state(Round::new(1)),
        },
    ];

    run_steps(&mut driver, steps)
}

#[test]
fn driver_steps_polka_previous_locked_vote_last_l32() {
    let value1 = Value::new(9999);
    let value2 = Value::new(8888);
    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([2, 2, 3]);
    let (_my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "(v2): Starts round 0 (v1 is proposer), starts timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "(v2): Timeout propose expires in round 0",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "(v2): Receives a prevote(nil) from v2(itself) in round 0",
            input: prevote_nil_input(&my_addr),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "(v2): Receives a proposal(value1) from v1 in round 0",
            input: proposal_input(
                Round::new(0),
                value1.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "(v2): Receives prevote(value1) from v3 in round 0, starts timeout prevote",
            input: prevote_input(value1.clone(), &v3.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "(v2): Timeout prevote expires in round 0",
            input: timeout_prevote_input(Round::new(0)),
            expected_outputs: vec![precommit_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "(v2): Receives precommit(nil) from v2(itself) in round 0",
            input: precommit_nil_input(Round::new(0), &my_addr),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "(v2): Receives precommit(nil) from v3 in round 0, starts timeout precommit",
            input: precommit_nil_input(Round::new(0), &v3.address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "(v2): Timeout precommit expires in round 0",
            input: timeout_precommit_input(Round::new(0)),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        // Round 1
        TestStep {
            desc: "(v2): Starts round 1(v3 is proposer), starts timeout propose",
            input: new_round_input(Round::new(1), v3.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "(v2): Receives proposal(value2) from v3 in round 1",
            input: proposal_input(
                Round::new(1),
                value2.clone(),
                Round::Nil,
                Validity::Valid,
                v3.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(1), value2.clone(), &my_addr)],
            expected_round: Round::new(1),
            new_state: prevote_state(Round::new(1)),
        },
        TestStep {
            desc: "(v2): Receives prevote(value2) from v3 in round 1",
            input: prevote_input_at(Round::new(1), value2.clone(), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(1),
            new_state: prevote_state(Round::new(1)),
        },
        TestStep {
            desc: "(v2): Receives prevote(value2) from v2(itself) in round 1, locks value2",
            input: prevote_input_at(Round::new(1), value2.clone(), &v2.address),
            expected_outputs: vec![precommit_output(Round::new(1), value2.clone(), &my_addr)],
            expected_round: Round::new(1),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value2.clone(),
                    Round::Nil,
                    v3.address,
                ),
            ),
        },
        TestStep {
            desc: "(v2): Receives precommit(value2) from v2(itself) in round 1",
            input: precommit_input_at(Round::new(1), value2.clone(), &my_addr),
            expected_outputs: vec![],
            expected_round: Round::new(1),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value2.clone(),
                    Round::Nil,
                    v3.address,
                ),
            ),
        },
        TestStep {
            desc: "(v2): Receives precommit(nil) from v3 in round 1, start timeout precommit",
            input: precommit_nil_input(Round::new(1), &v3.address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value2.clone(),
                    Round::Nil,
                    v3.address,
                ),
            ),
        },
        TestStep {
            desc: "(v2): Timeout precommit expires in round 1",
            input: timeout_precommit_input(Round::new(1)),
            expected_outputs: vec![new_round_output(Round::new(2))],
            expected_round: Round::new(2),
            new_state: new_round_with_proposal_and_locked_and_valid(
                Round::new(2),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value2.clone(),
                    Round::Nil,
                    v3.address,
                ),
            ),
        },
        // Round 2
        TestStep {
            desc: "(v2): Starts round 2(v1 is proposer), starts timeout propose",
            input: new_round_input(Round::new(2), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(2))],
            expected_round: Round::new(2),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(2),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value2.clone(),
                    Round::Nil,
                    v3.address,
                ),
            ),
        },
        TestStep {
            desc: "(v2): Receives proposal(value1) from v1 in round 2",
            input: proposal_input(
                Round::new(2),
                value1.clone(),
                Round::new(0),
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(2),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(2),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value2.clone(),
                    Round::Nil,
                    v3.address,
                ),
            ),
        },
        TestStep {
            desc: "(v2): Receives prevote(value1) from v1 from round 0",
            input: prevote_input_at(Round::new(0), value1.clone(), &v1.address),
            expected_outputs: vec![prevote_nil_output(Round::new(2), &my_addr)],
            expected_round: Round::new(2),
            new_state: prevote_state_with_matching_proposal_and_locked_and_valid(
                Round::new(2),
                Proposal::new(
                    Height::new(1),
                    Round::new(1),
                    value2.clone(),
                    Round::Nil,
                    v3.address,
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
// L37 - v1 and v2 prevote v, v3 gets +2/3 prevotes, start timeout prevote (step prevote)
// L55 - Receive f+1 vote for round 1 from v2, start new round (step new_round)
// L21 - v3 is not proposer starts timeout propose (step propose)
// L28 - v3 receives invalid proposal and has 2f+1 prevotes from round 0 and:
//   L29 - with invalid proposal
//     L32 - v2 sends prevote(nil, round=1) (step prevote)
#[test]
fn driver_steps_polka_previous_invalid_proposal() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3, v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Timeout propopse, prevote nil (v3)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes a proposal",
            input: prevote_input(value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes same proposal, we get +2/3 prevotes, start prevote timer",
            input: prevote_input(value.clone(), &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v2",
            input: prevote_input_at(Round::new(1), value.clone(), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Receive an invalid proposal for POL round 0",
            input: proposal_input(
                Round::new(1),
                value.clone(),
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
// L24 - v3 receives proposal, prevotes value  (step prevote)
// L37 - v1 and v2 prevote v, v3 gets +2/3 prevotes (step precommit)
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
    let (_my_sk, my_addr) = (sk3, v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Receive a valid proposal for round 0",
            input: proposal_input(
                Round::new(0),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes the proposal",
            input: prevote_input(value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes same proposal, we get +2/3 prevotes, precommit",
            input: prevote_input(value.clone(), &v2.address),
            expected_outputs: vec![precommit_output(Round::new(0), value.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                Proposal::new(
                    Height::new(1),
                    Round::new(0),
                    value.clone(),
                    Round::Nil,
                    v1.address,
                ),
            ),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v2",
            input: prevote_input_at(Round::new(1), value.clone(), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(0),
                    value.clone(),
                    Round::Nil,
                    v1.address,
                ),
            ),
        },
        TestStep {
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                Proposal::new(
                    Height::new(1),
                    Round::new(0),
                    value.clone(),
                    Round::Nil,
                    v1.address,
                ),
            ),
        },
        TestStep {
            desc: "Receive a valid proposal for round 1 with different value",
            input: proposal_input(
                Round::new(1),
                other_value.clone(),
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
                    value.clone(),
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
// L59 - v2 receives timeout propose, prevotes nil (step prevote)
// L35 - v1 and v3 prevote some proposal(v), v2 gets +2/3 prevotes, starts timeout prevote (step prevote)
// L63 - v2 receives timeout prevote, prevotes nil (step precommit)
// L36 - v2 receives the proposal(v) from v1, sets valid = v (L42, L43) but does NOT lock (L37-L41 not executed) (step precommit)
// L56 - v2 receives a prevote(id(v), round=1) from v3, starts new round (step new_round)
//   Note - this doesn't seem correct v3 behaviour
// L16, L19 - v2 is the proposer and has a valid value from round 0, propose(round=1, value=v, valid_round=0) (step propose)
// L28 - v2 receives its proposal and has 2f+1 prevotes from round 0 and:
//   L29 - locked_round(-1) < valid_round(0) and valid_round(0) < round(1) BUT locked_value is nil
//     L32 - v2 sends prevote(nil, round=1) (step prevote)
#[test]
fn driver_steps_polka_previous_with_no_locked() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([2, 2, 3]);
    let (_my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Timeout propose, prevote nil (v2)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v3 prevotes some proposal",
            input: prevote_input(value.clone(), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes same proposal, we get +2/3 prevotes, start timeout prevote",
            input: prevote_input(value.clone(), &v1.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout prevote, prevote nil (v2)",
            input: timeout_prevote_input(Round::new(0)),
            expected_outputs: vec![precommit_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive  a proposal - L36, we don't lock, we set valid",
            input: proposal_input(
                Round::new(0),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v3.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_valid(
                Round::new(0),
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::Nil, v3.address),
            ),
        },
        TestStep {
            desc: "Receive f+1 vote for round 1 from v3",
            input: prevote_input_at(Round::new(1), value.clone(), &v3.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_valid(
                Round::new(1),
                Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::Nil, v3.address),
            ),
        },
        TestStep {
            desc: "start round 1, we are proposer with a valid value from round 0, propose it",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![proposal_output(
                Round::new(1),
                value.clone(),
                Round::new(0),
                v2.address,
            )],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_valid(
                Round::new(1),
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::Nil, v2.address),
            ),
        },
        TestStep {
            desc: "Receive our own proposal, prevote value even if we are not locked on it (L29 with lockedRoundp == nil < vr)",
            input: proposal_input(
                Round::new(1),
                value.clone(),
                Round::new(0),
                Validity::Valid,
                v2.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(1), value.clone(), &my_addr)],
            expected_round: Round::new(1),
            new_state: prevote_state_with_proposal_and_valid(
                Round::new(1),
                Round::new(0),
                Proposal::new(Height::new(1), Round::new(0), value.clone(), Round::Nil, v2.address),
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
// L34 - v3 gets +2/3 prevotes nil (from v1 and v2), event ignored (step propose)
// L57 - v3 receives timeout propose, prevotes nil (step prevote)
// L44 - polkaNil is replayed and v3 precommits nil (step precommit)
#[test]
fn driver_steps_polka_nil_and_timeout_propose() {
    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "v2 prevotes nil, we get polkaNil, but we are in Propose step",
            input: prevote_nil_input(&v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propose, prevote nil then, since we hit L44, precommit nil",
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
// L57 - v3 receives proposal, prevotes value  (step prevote)
// L36 - polka is replayed and v3 precommits the value (step precommit)
#[test]
fn driver_steps_polka_value_then_proposal() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "v1 prevotes a proposal",
            input: prevote_input(value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes same proposal, we get +2/3 prevotes, but we are in Propose step",
            input: prevote_input(value.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive a proposal from v1 - L22 send prevote",
            input: proposal_input(
                Round::new(0),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![
                prevote_output(Round::new(0), value.clone(), &my_addr),
                precommit_output(Round::new(0), value.clone(), &my_addr),
            ],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                Proposal::new(
                    Height::new(1),
                    Round::new(0),
                    value.clone(),
                    Round::Nil,
                    v1.address,
                ),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
fn driver_steps_polka_any_then_proposal_other() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "v2 prevotes same proposal, we get polkaAny, but we are in Propose step",
            input: prevote_input(value.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive a proposal from v1 - L22 send prevote, replay polkaAny, start timeout prevote",
            input: proposal_input(Round::new(0), value.clone(), Round::Nil, Validity::Valid, v1.address),
            expected_outputs: vec![
                prevote_output(Round::new(0),value.clone(), &my_addr),
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
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value1.clone(),
        Round::Nil,
        v1.address,
    );

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive a proposal from v1, start timeout prevote",
            input: proposal_input(
                Round::new(0),
                value1.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value1.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes proposal",
            input: prevote_input(value1.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes different value",
            input: prevote_input(value2, &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes proposal, v3 precommits with proposal and locked and valid",
            input: prevote_input(value1.clone(), &v1.address),
            expected_outputs: vec![precommit_output(Round::new(0), value1.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(Round::new(0), proposal),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
fn driver_equivocating_proposer_others_vote_first() {
    let value1 = Value::new(9999);
    let value2 = Value::new(42);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value1.clone(),
        Round::Nil,
        v1.address,
    );

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 1 from v1, start timeout prevote",
            input: proposal_input(
                Round::new(0),
                value1.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value1.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 2 from v1",
            input: proposal_input(
                Round::new(0),
                value2.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes proposal 1",
            input: prevote_input(value1.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes proposal 1, v3 precommits with proposal and locked and valid",
            input: prevote_input(value1.clone(), &v1.address),
            expected_outputs: vec![precommit_output(Round::new(0), value1.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

// Prevote on one value but precommit and decides on another value in the same round with a Byzantine proposer that equivocates
//
// Ev:             NewRound(0)           Proposal(v1)           Proposal(v2)          PolkaValue(v2)            PrecommitValue(v2)
// State: Init ----------------> Propose ------------> Prevote -------------> Prevote --------------> Precommit -----------------> Commit (v2)
//
// Msg:           propose_timer           prevote(v1)               None               precommit(v2)                   None
// Alg:               L21                    L24                    L24                    L36                         L49
//
// v1=2, v2=3, v3=2, we are v3
// L21 - v3 is not proposer, starts propose timer (step propose)
// L24 - v3 receives proposal v1 and prevotes for it (step prevote)
// L24 - v3 receives proposal v2 but does not vote because step is prevote
// L36 - v3 receives proposal and polka for proposal v2, sends precommit for v2 and update valid and locked values (step precommit)
// L49 - v3 receives precommits for v2 from v1 and v2, reaches 2f+1 precommits, and decides v2
#[test]
fn driver_equivocating_proposer_others_vote_second() {
    let value1 = Value::new(9999);
    let value2 = Value::new(42);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value2.clone(),
        Round::Nil,
        v1.address,
    );

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 1 from v1",
            input: proposal_input(
                Round::new(0),
                value1.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value1.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v3 prevotes proposal 1",
            input: prevote_input(value1.clone(), &v3.address),
            //expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 2 from v1",
            input: proposal_input(
                Round::new(0),
                value2.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes proposal 2",
            input: prevote_input(value2.clone(), &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes proposal 2, v3 precommits with proposal and locked and valid",
            input: prevote_input(value2.clone(), &v1.address),
            expected_outputs: vec![precommit_output(Round::new(0), value2.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "v2 precommits proposal 2",
            input: precommit_input(Round::new(0), value2.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "v1 precommits proposal 2",
            input: precommit_input(Round::new(0), value2.clone(), &v1.address),
            expected_outputs: vec![decide_output(Round::new(0), proposal.clone())],
            expected_round: Round::new(0),
            new_state: decided_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

// Prevote for one value, but update the valid value to another after receiving a polka for it in the same round by receiving individual votes.
// The Byzantine proposer proposed two values in the same round.
// Ev:             NewRound(0)           Proposal(v1)             PolkaAny            Proposal(v2)        TimeoutPrevoteExpire           PolkaValue(v2)
// State: Init ----------------> Propose ------------> Prevote -------------> Prevote ------------> Prevote ---------------> Precommit -----------------> Precommit + valid_value
//
// Msg:           propose_timer           prevote(v1)           prevote_timer             None               precommit(nil)                 None
// Alg:               L21                    L24                     L35                  L24                     L61                       L36
//
// v1=2, v2=3, v3=2, we are v3
// L21 - v3 is not proposer, starts propose timer (step propose)
// L24 - v3 receives proposal v1 and prevotes for it (step prevote)
// L35 - v3 receives prevote from itself and prevote from v2 but for different values, polkaAny, starts prevote timer
// L24 - v3 receives proposal v2 but does not vote for it because step is prevote
// L61 - timer prevote expires on v3, sends precommit nil (step precommit)
// L36 - v3 receives prevote from v2, polkaValue, only updates valid value and round because step is precommit
#[test]
fn driver_equivocating_proposer_valid_value() {
    let value1 = Value::new(9999);
    let value2 = Value::new(42);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value2.clone(),
        Round::Nil,
        v1.address,
    );

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, as v3, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 1 from v1",
            input: proposal_input(
                Round::new(0),
                value1.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value1.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive prevote for proposal 1 from itself",
            input: prevote_input(value1.clone(), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive prevote for proposal 2 from v2, trigger timeout prevote",
            input: prevote_input(value2.clone(), &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 2 from v1",
            input: proposal_input(
                Round::new(0),
                value2.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "timeout prevote expires, send precommit nil",
            input: timeout_prevote_input(Round::new(0)),
            expected_outputs: vec![precommit_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive prevote for proposal 2 from v1, update valid value to v2",
            input: prevote_input(value2.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_valid(
                Round::new(0),
                Round::new(0),
                proposal.clone(),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

// Prevote for one value, but update the valid value to another after receiving a **polka certificate** for it in the same round.
// The polka certificate includes an equivocated vote from a Byzantine proposer who proposed both values in the same round.
//
// Ev:             NewRound(0)           Proposal(v1)             PolkaAny            Proposal(v2)        TimeoutPrevoteExpire           PolkaValue(v2)
// State: Init ----------------> Propose ------------> Prevote -------------> Prevote ------------> Prevote ---------------> Precommit -----------------> Precommit + valid_value
//
// Msg:           propose_timer           prevote(v1)           prevote_timer             None               precommit(nil)                 None
// Alg:               L21                    L24                     L35                  L24                     L61                       L36
//
// v1=2, v2=3, v3=2, we are v3
// L21 - v3 is not proposer, starts propose timer (step propose)
// L24 - v3 receives proposal v1 and prevotes for it (step prevote)
// L35 - v3 receives prevote from itself and prevote from v2 but for different values, polkaAny, starts prevote timer
// L24 - v3 receives proposal v2 but does not vote for it because step is prevote
// L61 - timer prevote expires on v3, sends precommit nil (step precommit)
// L36 - v3 receives polka certificate for v1 with equivocated vote from v2, polkaValue, only updates valid value and round because step is precommit
#[test]
fn driver_equivocating_proposer_valid_value_with_polka_cert() {
    let value1 = Value::new(9999);
    let value2 = Value::new(42);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value1.clone(),
        Round::Nil,
        v1.address,
    );

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, as v3, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 1 from v1",
            input: proposal_input(
                Round::new(0),
                value1.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value1.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive prevote for proposal 1 from itself",
            input: prevote_input(value1.clone(), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive prevote for proposal 2 from v2, trigger timeout prevote",
            input: prevote_input(value2.clone(), &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 2 from v1",
            input: proposal_input(
                Round::new(0),
                value2.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "timeout prevote expires, send precommit nil",
            input: timeout_prevote_input(Round::new(0)),
            expected_outputs: vec![precommit_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive polka certificate for proposal 1",
            input: polka_certificate_input_at(
                Round::new(0),
                value1.clone(),
                &[v1.address, v2.address],
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_valid(
                Round::new(0),
                Round::new(0),
                proposal.clone(),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
#[should_panic(expected = "BUG: Received proposals from different validators in the same round")]
fn driver_conflicting_proposal_panic() {
    let value1 = Value::new(9999);
    let value2 = Value::new(42);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 1 from v1, start timeout prevote",
            input: proposal_input(
                Round::new(0),
                value1.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![prevote_output(Round::new(0), value1.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal 2 from v2",
            input: proposal_input(
                Round::new(0),
                value2,
                Round::Nil,
                Validity::Valid,
                v2.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
fn driver_step_change_mux_with_proposal() {
    let value: Value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "v2 prevotes nil, we get polkaNil, but we are in Propose step",
            input: prevote_nil_input(&v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propose, prevote nil then precommit nil",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![
                prevote_nil_output(Round::new(0), &my_addr),
                precommit_nil_output(Round::new(0), &my_addr),
            ],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 precommits nil",
            input: precommit_nil_input(Round::new(0), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits nil, we get polkaNil, but we are in Propose step",
            input: precommit_nil_input(Round::new(0), &v2.address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal for next round, store",
            input: proposal_input(
                Round::new(1),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout precommit, start new round",
            input: timeout_precommit_input(Round::new(0)),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc:
                "Start round 1, change step to propose, start propose timer, mux proposal, prevote",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![
                start_propose_timer_output(Round::new(1)),
                prevote_output(Round::new(1), value.clone(), &my_addr),
            ],
            expected_round: Round::new(1),
            new_state: prevote_state(Round::new(1)),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
fn driver_step_change_mux_with_proposal_and_polka() {
    let value: Value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(1),
        value.clone(),
        Round::Nil,
        v1.address,
    );

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
            desc: "Receive proposal for next round, store",
            input: proposal_input(
                Round::new(1),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes value for round 1",
            input: prevote_input_at(Round::new(1), value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes value for round 1, we hit the f+1 threshold, move to round 1",
            input: prevote_input_at(Round::new(1), value.clone(), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc:
                "Start round 1, change step to propose, start propose timer, mux proposal, prevote",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![
                start_propose_timer_output(Round::new(1)),
                prevote_output(Round::new(1), value.clone(), &my_addr),
                start_prevote_timer_output(Round::new(1)),
                precommit_output(Round::new(1), value.clone(), &my_addr),
            ],
            expected_round: Round::new(1),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal.clone(),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
fn driver_step_change_mux_with_proposal_and_commit_quorum() {
    let value: Value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(1),
        value.clone(),
        Round::Nil,
        v1.address,
    );

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
            desc: "Receive proposal for round 1, store",
            input: proposal_input(
                Round::new(1),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 precommits value for round 1",
            input: precommit_input_at(Round::new(1), value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits value for round 1, we hit f+1 threshold, move to round 1",
            input: precommit_input_at(Round::new(1), value.clone(), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc:
                "Start round 1, change step to propose, start propose timer, mux proposal, decide",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![
                start_propose_timer_output(Round::new(1)),
                decide_output(Round::new(1), proposal),
            ],
            expected_round: Round::new(1),
            new_state: decided_state(Round::new(1), Round::new(1), value),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
fn proposal_mux_with_polka() {
    let value: Value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(1),
        value.clone(),
        Round::Nil,
        v1.address,
    );

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
            desc: "v1 prevotes value for round 1",
            input: prevote_input_at(Round::new(1), value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes value for round 1, we hit the f+1 threshold, move to round 1",
            input: prevote_input_at(Round::new(1), value.clone(), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1, change step to propose, start propose timer",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Receive proposal for next round, store",
            input: proposal_input(
                Round::new(1),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![
                prevote_output(Round::new(1), value.clone(), &my_addr),
                precommit_output(Round::new(1), value.clone(), &my_addr),
            ],
            expected_round: Round::new(1),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal.clone(),
            ),
        },
    ];

    run_steps(&mut driver, steps);
}

#[test]
fn proposal_mux_with_commit_quorum() {
    let value: Value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(1),
        value.clone(),
        Round::Nil,
        v1.address,
    );

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
            desc: "v1 precommits value for round 1",
            input: precommit_input_at(Round::new(1), value.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc:
                "v2 precommits value for round 1, we hit f+1 threshold (also 2f+1), move to round 1",
            input: precommit_input_at(Round::new(1), value.clone(), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![
                start_propose_timer_output(Round::new(1)),
                start_precommit_timer_output(Round::new(1)),
            ],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Receive proposal for round 1, store, mux, decide",
            input: proposal_input(
                Round::new(1),
                value.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![decide_output(Round::new(1), proposal)],
            expected_round: Round::new(1),
            new_state: decided_state(Round::new(1), Round::new(1), value),
        },
    ];

    run_steps(&mut driver, steps);
}

// Test for bug with incorrect precommit when receiving proposal after polka for different value
// Scenario: Process receives polka for value v, then receives proposal for different value v'
// while in prevote step, and would incorrectly precommits v'.
//
// Ev:             NewRound(0)           Timeout(propose)          <polka(v)>               Proposal(v')
// State: NewRound ------------> Propose ----------------> Prevote ------------> Prevote ---------------> Precommit
// Msg:            propose_timer         Prevote(nil)              None                   Precommit(v')
#[test]
fn no_precommit_after_polka_and_proposal_different_values() {
    let value_v = Value::new(9999);
    let value_v_prime = Value::new(8888);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Timeout propose expires, prevote nil (v3)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes value v",
            input: prevote_input(value_v.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes value v, we get +2/3 prevotes (polka for v), start prevote timer",
            input: prevote_input(value_v.clone(), &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal for different value v', no output, stay in prevote",
            input: proposal_input(
                Round::new(0),
                value_v_prime.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
            // Former incorrect behavior:
            // expected_outputs: vec![precommit_output(Round::new(0), value_v_prime.clone(), &my_addr)],
            // new_state: precommit_state_with_proposal_and_locked_and_valid(
            //      Round::new(0),
            //      Proposal::new(
            //          Height::new(1),
            //          Round::new(0),
            //          value_v_prime.clone(),
            //          Round::Nil,
            //          v1.address,
            //      )
            // )
        },
    ];

    run_steps(&mut driver, steps);
}

// Test for bug with incorrect valid value update when receiving proposal after polka for different value
// Scenario: Process receives polka for value v, moves to precommit (with nil),
// then receives proposal for different value v' and would incorrectly updates valid value to v'
//
// Ev:             NewRound(0)           Timeout(propose)          <polka(v)>               Timeout(prevote)            Proposal(v')
// State: NewRound ------------> Propose ----------------> Prevote ------------> Prevote -----------------> Precommit -----------> Precommit
// Msg:            propose_timer         Prevote(nil)              prevote_timer         Precommit(nil)               None
#[test]
fn no_valid_update_after_polka_and_proposal_different_values() {
    let value_v = Value::new(9999);
    let value_v_prime = Value::new(8888);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Timeout propose expires, prevote nil (v3)",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 prevotes value v",
            input: prevote_input(value_v.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes value v, we get +2/3 prevotes (polka for v), start prevote timer",
            input: prevote_input(value_v.clone(), &v2.address),
            expected_outputs: vec![start_prevote_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout prevote expires, precommit nil since no proposal was received",
            input: timeout_prevote_input(Round::new(0)),
            expected_outputs: vec![precommit_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal for different value v', stays in precommit, no valid update",
            input: proposal_input(
                Round::new(0),
                value_v_prime.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
            // Former incorrect behavior:
            // new_state: precommit_state_with_proposal_and_valid(
            //     Round::new(0),
            //     Round::new(0),
            //     Proposal::new(
            //         Height::new(1),
            //         Round::new(0),
            //         value_v_prime.clone(),
            //         Round::Nil,
            //         v1.address,
            //     )
            // )
        },
    ];

    run_steps(&mut driver, steps);
}

// Test for incorrect prevote when receiving proposal with POL round after having polka for different value
// Scenario: Process receives polka for value v in round r, moves to round r' > r,
// then receives proposal for different value v' with POL round r and incorrectly prevotes v'
//
// Ev:             NewRound(0)           <polka(v)>            <f+1 for round 1>            NewRound(1)              Proposal(v', pol=0)
// State: NewRound ------------> Propose ------------> Propose -----------------> NewRound ------------> Propose -----------------------> Prevote
// Msg:            propose_timer                                  new_round(1)              propose_timer            Prevote(v')
#[test]
fn no_prevote_for_proposal_with_polka_for_different_value() {
    let value_v = Value::new(9999);
    let value_v_prime = Value::new(8888);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "v1 prevotes value v in round 0",
            input: prevote_input(value_v.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes value v in round 0, we get +2/3 prevotes (polka for v)",
            input: prevote_input(value_v.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 sends vote for round 1, we get f+1 votes, move to round 1",
            input: prevote_input_at(Round::new(1), Value::new(7777), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Receive proposal for v' with POL round 0, stays in propose, no vote",
            input: proposal_input(
                Round::new(1),
                value_v_prime.clone(),
                Round::new(0),
                Validity::Valid,
                v2.address,
            ),
            expected_round: Round::new(1),
            expected_outputs: vec![],
            new_state: propose_state(Round::new(1)),
            // Former incorrect behavior:
            // expected_outputs: vec![prevote_output(Round::new(1), value_v_prime.clone(), &my_addr)],
            // new_state: prevote_state(Round::new(1)),
        },
    ];

    run_steps(&mut driver, steps);
}

// Alternative test using polka certificate instead of individual votes
#[test]
fn no_prevote_for_proposal_with_polka_for_different_value_via_certificate() {
    let value_v = Value::new(9999);
    let value_v_prime = Value::new(8888);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Receive polka certificate for value v in round 0",
            input: polka_certificate_input_at(
                Round::new(0),
                value_v.clone(),
                &[v1.address, v2.address],
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 sends vote for round 1, we get f+1 votes, move to round 1",
            input: prevote_input_at(Round::new(1), Value::new(7777), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Receive proposal for v' with POL round 0, stays in propose, no vote",
            input: proposal_input(
                Round::new(1),
                value_v_prime.clone(),
                Round::new(0),
                Validity::Valid,
                v2.address,
            ),
            expected_round: Round::new(1),
            expected_outputs: vec![],
            new_state: propose_state(Round::new(1)),
            // Former incorrect behavior:
            // expected_outputs: vec![prevote_output(Round::new(1), value_v_prime.clone(), &my_addr)],
            // new_state: prevote_state(Round::new(1)),
        },
    ];

    run_steps(&mut driver, steps);
}

// Test for incorrect prevote nil when receiving invalid proposal with POL round after having polka
// Scenario: Process receives polka for value v in round r, moves to round r' > r,
// then receives invalid proposal for different value v' with POL round r and incorrectly prevotes nil
//
// Ev:             NewRound(0)           <polka(v)>               <f+1 for round 1>            NewRound(1)              Invalid Proposal(v', pol=0)
// State: NewRound ------------> Propose ------------> Propose -----------------> NewRound ------------> Propose ---------------------------> Prevote
// Msg:            propose_timer                                  new_round(1)             propose_timer                 Prevote(nil)
#[test]
fn no_prevote_for_invalid_proposal_with_polka_for_different_value() {
    let value_v = Value::new(9999);
    let value_v_prime = Value::new(8888);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "v1 prevotes value v in round 0",
            input: prevote_input(value_v.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 prevotes value v in round 0, we get +2/3 prevotes (polka for v)",
            input: prevote_input(value_v.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 sends vote for round 1, we get f+1 votes, move to round 1",
            input: prevote_input_at(Round::new(1), Value::new(7777), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Receive invalid proposal for v' with POL round 0, stays in propose, no vote",
            input: proposal_input(
                Round::new(1),
                value_v_prime.clone(),
                Round::new(0),
                Validity::Invalid, // Invalid proposal
                v2.address,
            ),
            expected_round: Round::new(1),
            expected_outputs: vec![],
            new_state: propose_state(Round::new(1)),
            // Former incorrect behavior:
            // expected_outputs: vec![prevote_nil_output(Round::new(1), &my_addr)],
            // new_state: prevote_state(Round::new(1)),
        },
    ];

    run_steps(&mut driver, steps);
}

// Alternative test using polka certificate instead of individual votes
#[test]
fn no_prevote_for_invalid_proposal_with_polka_for_different_value_via_certificate() {
    let value_v = Value::new(9999);
    let value_v_prime = Value::new(8888);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Receive polka certificate for value v in round 0",
            input: polka_certificate_input_at(
                Round::new(0),
                value_v.clone(),
                &[v1.address, v2.address],
            ),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 sends vote for round 1, we get f+1 votes, move to round 1",
            input: prevote_input_at(Round::new(1), Value::new(7777), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Receive invalid proposal for v' with POL round 0, stays in propose, no vote",
            input: proposal_input(
                Round::new(1),
                value_v_prime.clone(),
                Round::new(0),
                Validity::Invalid, // Invalid proposal
                v2.address,
            ),
            expected_round: Round::new(1),
            expected_outputs: vec![],
            new_state: propose_state(Round::new(1)),
            // Former incorrect behavior:
            // expected_outputs: vec![prevote_nil_output(Round::new(1), &my_addr)],
            // new_state: prevote_state(Round::new(1)),
        },
    ];

    run_steps(&mut driver, steps);
}

// It should not matter the order: first the proposal or first the polka certificate.
// In round 0, we first receive the proposal; in round 1, we first receive the polka certificate.
#[test]
fn prevote_and_precommit_upon_proposal_and_polka_cert() {
    let value_v = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let proposal0 = Proposal::new(
        Height::new(1),
        Round::new(0),
        value_v.clone(),
        Round::Nil,
        v2.address,
    );

    let proposal1 = Proposal::new(
        Height::new(1),
        Round::new(1),
        value_v.clone(),
        Round::Nil,
        v2.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive proposal for v, prevote(0,v)",
            input: proposal_input(
                Round::new(0),
                value_v.clone(),
                Round::Nil,
                Validity::Valid,
                v2.address,
            ),
            expected_round: Round::new(0),
            expected_outputs: vec![prevote_output(Round::new(0), value_v.clone(), &my_addr)],
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive polka certificate for v at round 0, precommit",
            input: polka_certificate_input_at(
                Round::new(0),
                value_v.clone(),
                &[v1.address, v2.address],
            ),
            expected_outputs: vec![precommit_output(Round::new(0), value_v.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal0.clone(),
            ),
        },
        TestStep {
            desc: "v2 sends vote for round 1, we get f+1 votes, move to round 1",
            input: prevote_input_at(Round::new(1), value_v.clone(), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal0.clone(),
            ),
        },
        TestStep {
            desc: "Start round 1, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(1), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal0.clone(),
            ),
        },
        TestStep {
            desc: "Receive polka certificate for v at round 1, do nothing right now",
            input: polka_certificate_input_at(
                Round::new(1),
                value_v.clone(),
                &[v1.address, v2.address],
            ),
            expected_outputs: vec![],
            expected_round: Round::new(1),
            new_state: propose_state_with_proposal_and_locked_and_valid(
                Round::new(1),
                proposal0.clone(),
            ),
        },
        TestStep {
            desc: "Receive proposal for v at round 1, prevote and precommit",
            input: proposal_input(
                Round::new(1),
                value_v.clone(),
                Round::Nil,
                Validity::Valid,
                v1.address,
            ),
            expected_round: Round::new(1),
            expected_outputs: vec![
                prevote_output(Round::new(1), value_v.clone(), &my_addr),
                precommit_output(Round::new(1), value_v.clone(), &my_addr),
            ],
            new_state: precommit_state_with_proposal_and_locked_and_valid(Round::new(1), proposal1),
        },
    ];

    run_steps(&mut driver, steps);
}

// Upon receiving 2f + 1 prevotes for nil, a process should precommit nil once it reaches the
// prevote step. This `PolkaNil` condition, however, can be produced during the propose step; once
// the prevote step is reached, the pending `PolkaNil` must be consumed.
#[test]
fn polka_nil_and_prevote_step_precommit_nil() {
    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Prevote from v2 while in propose step",
            input: prevote_nil_input(&v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Prevote from v3, we have PolkaNil, not triggered in propose step",
            input: prevote_nil_input(&v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v2",
            input: precommit_nil_input(Round::new(0), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v3, quorum of precommits, timeout to be scheduled",
            input: precommit_nil_input(Round::new(0), &v3.address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propose, prevote nil; since we have PolkaNil, also precommit nil",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![
                prevote_nil_output(Round::new(0), &my_addr),
                precommit_nil_output(Round::new(0), &my_addr),
                start_precommit_timer_output(Round::new(0)),
                start_precommit_timer_output(Round::new(0)),
            ],
            expected_round: Round::new(0),
            new_state: precommit_state(Round::new(0)),
        },
    ];

    run_steps(&mut driver, steps);
}

// Upon receiving 2f + 1 prevotes for different values, a process should schedule a prevote timeout
// once it reaches the prevote step. This `PolkaAny` condition, however, can be produced during the
// propose step; once the prevote step is reached, the pending `PolkaAny` must be consumed.
#[test]
fn polka_any_and_prevote_step_timeout_prevote() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Prevote from v2 while in propose step",
            input: prevote_nil_input(&v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Prevote from v3, we have Polka, not triggered in propose step",
            input: prevote_input_at(Round::new(0), value.clone(), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v2",
            input: precommit_nil_input(Round::new(0), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v3, quorum of precommits, timeout to be scheduled",
            input: precommit_nil_input(Round::new(0), &v3.address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propose, prevote nil; since we have PolkaAny, schedule timeout prevote",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![
                prevote_nil_output(Round::new(0), &my_addr),
                start_prevote_timer_output(Round::new(0)),
                start_precommit_timer_output(Round::new(0)),
            ],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
    ];

    run_steps(&mut driver, steps);
}

// A pending `PolkaAny` can also be produced upon receiving 2f + 1 prevotes for a value, whose
// associated proposal is missing. This `PolkaAny` condition, however, can be produced during the
// propose step; once the prevote step is reached, the pending `PolkaAny` must be consumed.
#[test]
fn polka_value_no_proposal_and_prevote_step_timeout_prevote() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Prevote from v2 while in propose step",
            input: prevote_input_at(Round::new(0), value.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Prevote from v3, we have PolkaValue(v) but not Proposal(v)",
            input: prevote_input_at(Round::new(0), value.clone(), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v2",
            input: precommit_nil_input(Round::new(0), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v3, quorum of precommits, timeout to be scheduled",
            input: precommit_nil_input(Round::new(0), &v3.address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propose, prevote nil; since the Polka is incomplete, schedule timeout prevote",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![
                prevote_nil_output(Round::new(0), &my_addr),
                start_prevote_timer_output(Round::new(0)),
                start_precommit_timer_output(Round::new(0)),
            ],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
    ];

    run_steps(&mut driver, steps);
}

// Before actually starting round 1, a process receives 2f + 1 prevotes and 2f + 1 precomits of
// round 1. The first event leads to a round skip. Once round 1 is started and timeout propose
// expires, the pending `PolkaAny` event should lead to the scheduling of a timeout prevote.
#[test]
fn polka_any_precommit_any_future_round() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Prevote from v3 in round 1",
            input: prevote_input_at(Round::new(1), value.clone(), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v3 in round 1",
            input: precommit_nil_input(Round::new(1), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v2 in round 1, we have PrecommitAny(1)",
            input: precommit_nil_input(Round::new(1), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Prevote from v2 in round 1, we have PolkaValue(1, v)",
            input: prevote_input_at(Round::new(1), value.clone(), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1 and start timeout propose and commit",
            input: new_round_input(Round::new(1), v1.address),
            expected_outputs: vec![
                start_propose_timer_output(Round::new(1)),
                start_precommit_timer_output(Round::new(1)),
            ],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Timeout propose, goes to prevote, start timeout prevote",
            input: timeout_propose_input(Round::new(1)),
            expected_outputs: vec![
                prevote_nil_output(Round::new(1), &my_addr),
                start_prevote_timer_output(Round::new(1)),
                start_precommit_timer_output(Round::new(1)),
            ],
            expected_round: Round::new(1),
            new_state: prevote_state(Round::new(1)),
        },
    ];

    run_steps(&mut driver, steps);
}

// Before actually starting round 1, a process receives 2f + 1 prevotes and 2f + 1 precomits of
// precomits of round 1. The first event leads to a round skip. Once round 1 is started and timeout
// propose expires, the pending `PolkaNil` event should lead the process to precommit nil.
#[test]
fn polka_nil_precommit_any_future_round() {
    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "Prevote from v3 in round 1",
            input: prevote_nil_input_at(Round::new(1), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v3 in round 1",
            input: precommit_nil_input(Round::new(1), &v3.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Precommit from v2 in round 1, we have PrecommitAny(1)",
            input: precommit_nil_input(Round::new(1), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(1))],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Prevote from v2 in round 1, we have PolkaValue(1, v)",
            input: prevote_nil_input_at(Round::new(1), &v2.address),
            expected_outputs: vec![],
            expected_round: Round::new(1),
            new_state: new_round(Round::new(1)),
        },
        TestStep {
            desc: "Start round 1 and start timeout propose and commit",
            input: new_round_input(Round::new(1), v1.address),
            expected_outputs: vec![
                start_propose_timer_output(Round::new(1)),
                start_precommit_timer_output(Round::new(1)),
            ],
            expected_round: Round::new(1),
            new_state: propose_state(Round::new(1)),
        },
        TestStep {
            desc: "Timeout propose, goes to prevote, precommit nil",
            input: timeout_propose_input(Round::new(1)),
            expected_outputs: vec![
                prevote_nil_output(Round::new(1), &my_addr),
                precommit_nil_output(Round::new(1), &my_addr),
                start_precommit_timer_output(Round::new(1)),
                start_precommit_timer_output(Round::new(1)), // :'(
            ],
            expected_round: Round::new(1),
            new_state: precommit_state(Round::new(1)),
        },
    ];

    run_steps(&mut driver, steps);
}

// A Proposal for an invalid value in round 1 with POL round 0 is received, then a matching polka
// certificate is received. Despite the global validation, the process should prevote nil. Before
// that we receive a non matching polka certificate, for round 1, that should be ignored.
#[test]
fn proposal_invalid_value_and_matching_pol_certificate() {
    let value_v = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
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
            desc: "v2 sends vote for round 2, we get f+1 votes, move to round 2",
            input: prevote_input_at(Round::new(2), Value::new(7777), &v2.address),
            expected_outputs: vec![new_round_output(Round::new(2))],
            expected_round: Round::new(2),
            new_state: new_round(Round::new(2)),
        },
        TestStep {
            desc: "Start round 2, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(2), v2.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(2))],
            expected_round: Round::new(2),
            new_state: propose_state(Round::new(2)),
        },
        TestStep {
            desc: "Receive invalid proposal for v with POL round 0, stays in propose, no vote",
            input: proposal_input(
                Round::new(2),
                value_v.clone(),
                Round::new(0),
                Validity::Invalid,
                v2.address,
            ),
            expected_round: Round::new(2),
            expected_outputs: vec![],
            new_state: propose_state(Round::new(2)),
        },
        TestStep {
            desc: "Receive a Polka for v in round 1, ignore",
            input: polka_certificate_input_at(
                Round::new(1),
                value_v.clone(),
                &[v1.address, v2.address],
            ),
            expected_round: Round::new(2),
            expected_outputs: vec![],
            new_state: propose_state(Round::new(2)),
        },
        TestStep {
            desc: "Receive a Polka for v in round 0, prevote nil",
            input: polka_certificate_input_at(
                Round::new(0),
                value_v.clone(),
                &[v1.address, v2.address],
            ),
            expected_round: Round::new(2),
            expected_outputs: vec![prevote_nil_output(Round::new(2), &my_addr)],
            new_state: prevote_state(Round::new(2)),
        },
    ];

    run_steps(&mut driver, steps);
}

// A node receives a proposal that is reported as Invalid by the application. Then, for instance,
// after a bug fix, the same proposal is reported as Valid. From this point, it can be decided.
#[test]
fn invalid_proposal_becomes_valid_decision_via_proposal() {
    let value_v = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value_v.clone(),
        Round::Nil,
        v2.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propopse, prevote nil",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 precommits v",
            input: precommit_input(Round::new(0), value_v.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits v, we get +2/3 but no proposal",
            input: precommit_input(Round::new(0), value_v.clone(), &v2.address),
            expected_round: Round::new(0),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive a Proposal for v, which is deemed Invalid",
            input: proposal_input_from_proposal(proposal.clone(), Validity::Invalid),
            expected_round: Round::new(0),
            expected_outputs: vec![],
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive again a Proposal for v, still Invalid, duplicated",
            input: proposal_input_from_proposal(proposal.clone(), Validity::Invalid),
            expected_round: Round::new(0),
            expected_outputs: vec![],
            new_state: prevote_state(Round::new(0)),
        },
        // Assume here that the process crashed, and the validation function was updated.
        TestStep {
            desc: "Receive a Proposal for v, which is now deemed Valid",
            input: proposal_input_from_proposal(proposal.clone(), Validity::Valid),
            expected_round: Round::new(0),
            expected_outputs: vec![decide_output(Round::new(0), proposal)],
            new_state: decided_state(Round::new(0), Round::new(0), value_v),
        },
    ];

    run_steps(&mut driver, steps);
}

// A node receives a proposal that is reported as Invalid by the application. Then, for instance,
// after a bug fix, the same proposal is reported as Valid. From this point, it can be decided.
#[test]
fn invalid_proposal_becomes_valid_decision_via_votes() {
    let value_v = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value_v.clone(),
        Round::Nil,
        v2.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propopse, prevote nil",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive a Proposal for v, which is deemed Invalid",
            input: proposal_input_from_proposal(proposal.clone(), Validity::Invalid),
            expected_round: Round::new(0),
            expected_outputs: vec![],
            new_state: prevote_state(Round::new(0)),
        },
        // Assume here that the process crashed, and the validation function was updated.
        TestStep {
            desc: "Receive a Proposal for v, which is now deemed Valid",
            input: proposal_input_from_proposal(proposal.clone(), Validity::Valid),
            expected_round: Round::new(0),
            expected_outputs: vec![],
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v1 precommits v",
            input: precommit_input(Round::new(0), value_v.clone(), &v1.address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "v2 precommits v, we get +2/3 and decide v",
            input: precommit_input(Round::new(0), value_v.clone(), &v2.address),
            expected_outputs: vec![decide_output(Round::new(0), proposal)],
            expected_round: Round::new(0),
            new_state: decided_state(Round::new(0), Round::new(0), value_v),
        },
    ];

    run_steps(&mut driver, steps);
}

// A node receives a proposal that is reported as Invalid by the application. Then, for instance,
// after a bug fix, the same proposal is reported as Valid. From this point, it can be decided.
#[test]
fn invalid_proposal_becomes_valid_decision_via_certificate() {
    let value_v = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value_v.clone(),
        Round::Nil,
        v2.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer, start timeout propose",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Timeout propopse, prevote nil",
            input: timeout_propose_input(Round::new(0)),
            expected_outputs: vec![prevote_nil_output(Round::new(0), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "Receive a Proposal for v, which is deemed Invalid",
            input: proposal_input_from_proposal(proposal.clone(), Validity::Invalid),
            expected_round: Round::new(0),
            expected_outputs: vec![],
            new_state: prevote_state(Round::new(0)),
        },
        // Assume here that the process crashed, and the validation function was updated.
        TestStep {
            desc: "Receive a Proposal for v, which is now deemed Valid",
            input: proposal_input_from_proposal(proposal.clone(), Validity::Valid),
            expected_round: Round::new(0),
            expected_outputs: vec![],
            new_state: prevote_state(Round::new(0)),
        },
        // This should never happen. We should add an error message here.
        TestStep {
            desc: "Receive a Proposal for v, back to Invalid, should be ignored",
            input: proposal_input_from_proposal(proposal.clone(), Validity::Invalid),
            expected_round: Round::new(0),
            expected_outputs: vec![],
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "we get a certificate for v and decide v",
            input: commit_certificate_input_at(
                Round::new(0),
                value_v.clone(),
                &[v1.address, v2.address],
            ),
            expected_outputs: vec![decide_output(Round::new(0), proposal)],
            expected_round: Round::new(0),
            new_state: decided_state(Round::new(0), Round::new(0), value_v),
        },
    ];

    run_steps(&mut driver, steps);
}

// A decision via the synchronization protocol.
// A Commit certificate for v is received from the sync protocol and processed by the driver.
// Then the application receives the fully decided value from the sync protocol, validates it
// and produces ProposedValue event with Origin::Sync, processed by consensus-core.
// This is becomes a SyncDecision() driver input with a synthetic Proposal.
#[test]
fn sync_decision_certificate_then_proposal() {
    let value_v = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([2, 3, 2]);
    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs, my_addr, Default::default());

    let synthetic_proposal = Proposal::new(
        Height::new(1),
        Round::new(0),
        value_v.clone(),
        Round::Nil,
        v1.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input: new_round_input(Round::new(0), v1.address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        // NOTICE: if we receive the decision v from the Sync protocol or the WAL
        // **before** the Commit certificate nothing happens.
        // But once the certificate is received, the Sync protocol decision
        // input leads to the production of a SyncDecision() input.
        // We cannot reproduce this scenario here.
        TestStep {
            desc: "Certificate for v from Sync protocol",
            input: commit_certificate_input_at(
                Round::new(0),
                value_v.clone(),
                &[v1.address, v2.address],
            ),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "Decision value v from Sync protocol",
            input: Input::SyncDecision(synthetic_proposal.clone()),
            expected_outputs: vec![decide_output(Round::new(0), synthetic_proposal)],
            expected_round: Round::new(0),
            new_state: decided_state(Round::new(0), Round::new(0), value_v),
        },
    ];

    run_steps(&mut driver, steps);
}

// Commit certificate from driver after re-applied votes (round-certificate simulation).
//
// We are v2 (local node). Sequence:
// - Start round 0, receive proposal, send our (v2) prevote
// - Receive prevotes from v1, v3, we have a polka, we precommit
// - Receive precommit our (v2) precommit and v1's (we don't have +2/3 in round 0 yet) (simulates restart of v3 with proposal and prevote)
// - Receive v4's nil prevote and precommit (simulates restart of v4 without proposal)
// - Re-apply precommits v1, v2, v4 (simulate round certificate)
// - Late precommit from v3, we have a quorum and decide (simulates restart of v3 with proposal and prevote)
// - Build the commit certificate from the driver's received_votes and verify
#[test]
fn commit_certificate_from_driver_verifies_after_reapplied_votes_from_round_certificate() {
    const SEED: u64 = 0xfeedbeef;
    let validators_and_keys = make_validators_seeded([10, 10, 10, 10], SEED);
    let validators: Vec<_> = validators_and_keys.iter().map(|(v, _)| v.clone()).collect();
    let signers: Vec<Ed25519Provider> = validators_and_keys
        .iter()
        .map(|(_, sk)| Ed25519Provider::new(sk.clone()))
        .collect();
    let validator_set = ValidatorSet::new(validators.clone());
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);
    let value = Value::new(42);
    let value_id = value.id();
    let my_addr = validators[1].address; // we are v2

    // Precommits for value need real signatures for certificate verification at end.
    let precommit_v1 = block_on(signers[0].sign_vote(ctx.new_precommit(
        height,
        round,
        NilOrVal::Val(value_id),
        validators[0].address,
    )))
    .unwrap();
    let precommit_v2 = block_on(signers[1].sign_vote(ctx.new_precommit(
        height,
        round,
        NilOrVal::Val(value_id),
        validators[1].address,
    )))
    .unwrap();
    let precommit_v3 = block_on(signers[2].sign_vote(ctx.new_precommit(
        height,
        round,
        NilOrVal::Val(value_id),
        validators[2].address,
    )))
    .unwrap();

    let proposal = Proposal::new(
        height,
        round,
        value.clone(),
        Round::Nil,
        validators[0].address,
    );

    let mut driver = Driver::new(
        ctx.clone(),
        height,
        validator_set.clone(),
        my_addr,
        Default::default(),
    );

    let steps = vec![
        // Start and proposal
        TestStep {
            desc: "Start round 0, we (v2) are not the proposer",
            input: new_round_input(Round::new(0), validators[0].address),
            expected_outputs: vec![start_propose_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: propose_state(Round::new(0)),
        },
        TestStep {
            desc: "We receive the proposal (from v1)",
            input: proposal_input_from_proposal(proposal.clone(), Validity::Valid),
            expected_outputs: vec![prevote_output(Round::new(0), value.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "We (v2) get our prevote for value",
            input: prevote_input_at(round, value.clone(), &validators[1].address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        // +2/3 Prevotes, Polka
        TestStep {
            desc: "We receive v1's prevote for value",
            input: prevote_input_at(round, value.clone(), &validators[0].address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: prevote_state(Round::new(0)),
        },
        TestStep {
            desc: "We receive v3's prevote for value, we have a Polka, we (v2) precommit",
            input: prevote_input_at(round, value.clone(), &validators[2].address),
            expected_outputs: vec![precommit_output(Round::new(0), value.clone(), &my_addr)],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "We (v2) get our precommit for value",
            input: Input::Vote(precommit_v2.clone()),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        // Receive some Precommits (only v1 and v2 so far, < 2/3)
        TestStep {
            desc: "We receive v1's precommit for value",
            input: Input::Vote(precommit_v1.clone()),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        TestStep {
            desc:
                "We receive v4's precommit for nil (+2/3 precommits total, start precommit timer)",
            input: precommit_nil_input(round, &validators[3].address),
            expected_outputs: vec![start_precommit_timer_output(Round::new(0))],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        // Simulate round certificate with previous precommits by re-applying precommits: v1, v2, v4(nil)
        TestStep {
            desc: "Re-apply v1 precommit (simulate round certificate)",
            input: Input::Vote(precommit_v1.clone()),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "Re-apply v2 precommit (simulate round certificate)",
            input: Input::Vote(precommit_v2.clone()),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        TestStep {
            desc: "Re-apply v4 precommit (simulate round certificate)",
            input: precommit_nil_input(round, &validators[3].address),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: precommit_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
        // Late precommit triggers decide
        TestStep {
            desc: "We receive v3's late precommit, reached quorum, decide",
            input: Input::Vote(precommit_v3),
            expected_outputs: vec![decide_output(Round::new(0), proposal.clone())],
            expected_round: Round::new(0),
            new_state: decided_state_with_proposal_and_locked_and_valid(
                Round::new(0),
                proposal.clone(),
            ),
        },
    ];

    for step in steps {
        println!("Step: {}", step.desc);
        let outputs = driver.process(step.input).expect("execute succeeded");
        assert_eq!(outputs, step.expected_outputs, "expected outputs");
        assert_eq!(driver.round(), step.expected_round, "expected round");
        assert_eq!(driver.round_state(), &step.new_state, "expected state");
    }

    // Build certificate from restore_precommits and verify.
    let commits = driver.restore_precommits(round, &value_id);

    let certificate = CommitCertificate::new(height, round, value_id, commits);

    let result = block_on(signers[0].verify_commit_certificate(
        &ctx,
        &certificate,
        &validator_set,
        ThresholdParams::default(),
    ));

    assert!(
        result.is_ok(),
        "commit certificate built from driver vote keeper must verify: {:?}",
        result.err()
    );
}

fn run_steps(driver: &mut Driver<TestContext>, steps: Vec<TestStep>) {
    let mut last_step = Step::Unstarted;
    for step in steps {
        println!("Step: {}", step.desc);

        let outputs = driver.process(step.input).expect("execute succeeded");
        last_step = driver.round_state().step;

        assert_eq!(outputs, step.expected_outputs, "expected outputs");
        assert_eq!(driver.round(), step.expected_round, "expected round");
        assert_eq!(driver.round_state(), &step.new_state, "expected state");
    }

    // If we have a decision, move to the next height and check driver's state
    if last_step == Step::Commit {
        let next_height = driver.height().increment();
        driver.move_to_height(next_height, driver.validator_set().clone());
        check_driver_initial_state(driver, next_height);
    }
}

fn check_driver_initial_state(driver: &Driver<TestContext>, height: Height) {
    assert_eq!(driver.height(), height);
    assert_eq!(driver.round(), Round::Nil);
    assert_eq!(driver.round_state().step, Step::Unstarted);
    assert_eq!(driver.round_state().locked, None);
    assert_eq!(driver.round_state().valid, None);
    assert_eq!(driver.round_state().decision, None);

    assert_eq!(driver.proposer_address(), None);
    assert_eq!(driver.proposals().all_rounds().len(), 0);
    assert_eq!(driver.votes().rounds(), 0);
    assert_eq!(driver.commit_certificates().len(), 0);
    assert_eq!(driver.polka_certificates().len(), 0);
    assert_eq!(driver.pending_inputs().len(), 0);
    assert_eq!(driver.last_prevote(), None);
    assert_eq!(driver.last_precommit(), None);
    assert_eq!(driver.round_certificate(), None);
    assert_eq!(driver.proposer_address(), None);
    assert_eq!(driver.proposals().all_rounds().len(), 0);
    assert_eq!(driver.votes().rounds(), 0);
}
