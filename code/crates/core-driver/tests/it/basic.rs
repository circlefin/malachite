#![allow(clippy::needless_update)]

use std::sync::Arc;

use malachitebft_core_state_machine::state::{RoundValue, State, Step};
use malachitebft_core_types::{
    NilOrVal, Round, SignedProposal, SignedVote, Timeout, TimeoutKind, Validity,
};
use malachitebft_test::proposer_selector::{FixedProposer, ProposerSelector, RotateProposer};
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, ValidatorSet, Value, ValueId, Vote,
};

use informalsystems_malachitebft_core_driver::{Driver, Error, Input, Output};

pub struct TestStep {
    desc: &'static str,
    input: Option<Input<TestContext>>,
    expected_outputs: Vec<Output<TestContext>>,
    expected_round: Round,
    new_state: State<TestContext>,
}

fn new_signed_proposal(
    height: Height,
    round: Round,
    value: Value,
    pol_round: Round,
    address: Address,
) -> SignedProposal<TestContext> {
    SignedProposal::new(
        Proposal::new(height, round, value.clone(), pol_round, address),
        Signature::test(),
    )
}

fn new_signed_prevote(
    height: Height,
    round: Round,
    value: NilOrVal<ValueId>,
    addr: Address,
) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_prevote(height, round, value, addr),
        Signature::test(),
    )
}

fn new_signed_precommit(
    height: Height,
    round: Round,
    value: NilOrVal<ValueId>,
    addr: Address,
) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_precommit(height, round, value, addr),
        Signature::test(),
    )
}

pub fn output_to_input(
    output: Output<TestContext>,
    sel: &dyn ProposerSelector<TestContext>,
    vs: &ValidatorSet,
) -> Option<Input<TestContext>> {
    match output {
        Output::NewRound(height, round) => {
            let proposer = sel.select_proposer(height, round, vs);
            Some(Input::NewRound(height, round, proposer))
        }
        // Let's consider our own proposal to always be valid
        Output::Propose(p) => Some(Input::Proposal(
            SignedProposal::new(p, Signature::test()),
            Validity::Valid,
        )),
        Output::Vote(v) => Some(Input::Vote(SignedVote::new(v, Signature::test()))),
        Output::Decide(_, _) => None,
        Output::ScheduleTimeout(_) => None,
        Output::GetValue(_, _, _) => None,
    }
}

#[test]
fn driver_steps_proposer() {
    let value = Value::new(9999);

    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let sel = Arc::new(FixedProposer::new(my_addr));
    let vs = ValidatorSet::new(vec![v1, v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let proposal = new_signed_proposal(
        Height::new(1),
        Round::new(0),
        value.clone(),
        Round::Nil,
        my_addr,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are proposer, ask for a value to propose",
            input: Some(Input::NewRound(Height::new(1), Round::new(0), my_addr)),
            expected_outputs: vec![
                Output::ScheduleTimeout(Timeout::new(Round::new(0), TimeoutKind::Propose)),
                Output::GetValue(
                    Height::new(1),
                    Round::new(0),
                    Timeout::new(Round::new(0), TimeoutKind::Propose),
                ),
            ],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Feed a value to propose, propose that value",
            input: Some(Input::ProposeValue(Round::new(0), value.clone())),
            expected_outputs: vec![Output::Propose(proposal.message.clone())],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Receive our own proposal, prevote it (v1)",
            input: None,
            expected_outputs: vec![Output::Vote(Vote::new_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Receive our own prevote v1",
            input: None,
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc: "v2 prevotes our proposal",
            input: Some(Input::Vote(new_signed_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v2.address,
            ))),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc: "v3 prevotes our proposal, we get +2/3 prevotes, precommit it (v1)",
            input: Some(Input::Vote(new_signed_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v3.address,
            ))),
            expected_outputs: vec![Output::Vote(Vote::new_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                locked: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                ..Default::default()
            },
        },
        TestStep {
            desc: "v1 receives its own precommit",
            input: None,
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                locked: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                ..Default::default()
            },
        },
        TestStep {
            desc: "v2 precommits our proposal",
            input: Some(Input::Vote(new_signed_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v2.address,
            ))),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                locked: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                ..Default::default()
            },
        },
        TestStep {
            desc: "v3 precommits our proposal, we get +2/3 precommits, decide it (v1)",
            input: Some(Input::Vote(new_signed_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v3.address,
            ))),
            expected_outputs: vec![Output::Decide(Round::new(0), proposal.message)],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Commit,
                locked: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                decision: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                ..Default::default()
            },
        },
    ];

    run_steps(&mut driver, steps, sel.as_ref(), &vs);
}

#[test]
fn driver_steps_proposer_timeout_get_value() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let sel = Arc::new(FixedProposer::new(my_addr));
    let vs = ValidatorSet::new(vec![v1, v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are proposer, ask for a value to propose",
            input: Some(Input::NewRound(Height::new(1), Round::new(0), my_addr)),
            expected_outputs: vec![
                Output::ScheduleTimeout(Timeout::new(Round::new(0), TimeoutKind::Propose)),
                Output::GetValue(
                    Height::new(1),
                    Round::new(0),
                    Timeout::new(Round::new(0), TimeoutKind::Propose),
                ),
            ],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Receive a propose timeout",
            input: Some(Input::TimeoutElapsed(Timeout::propose(Round::new(0)))),
            expected_outputs: vec![Output::Vote(Vote::new_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Nil,
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                round: Round::new(0),
                height: Height::new(1),
                step: Step::Prevote,
                ..Default::default()
            },
        },
    ];

    run_steps(&mut driver, steps, sel.as_ref(), &vs);
}

#[test]
fn driver_steps_not_proposer_valid() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([1, 2, 3]);

    // Proposer is v1, so we are not the proposer
    let (_my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let sel = Arc::new(FixedProposer::new(v1.address));
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let proposal = new_signed_proposal(
        Height::new(1),
        Round::new(0),
        value.clone(),
        Round::Nil,
        v1.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input: Some(Input::NewRound(
                Height::new(1),
                Round::new(0),
                proposal.validator_address,
            )),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::propose(Round::new(0)))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Receive a proposal, prevote it (v2)",
            input: Some(Input::Proposal(proposal.clone(), Validity::Valid)),
            expected_outputs: vec![Output::Vote(Vote::new_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Receive our own prevote (v2)",
            input: None,
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc: "v1 prevotes its own proposal",
            input: Some(Input::Vote(new_signed_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v1.address,
            ))),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc: "v3 prevotes v1's proposal, v2 gets +2/3 prevotes, precommits it",
            input: Some(Input::Vote(new_signed_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v3.address,
            ))),
            expected_outputs: vec![Output::Vote(Vote::new_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                locked: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                ..Default::default()
            },
        },
        TestStep {
            desc: "we receive our own precommit",
            input: None,
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                locked: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                ..Default::default()
            },
        },
        TestStep {
            desc: "v1 precommits its proposal",
            input: Some(Input::Vote(new_signed_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v1.address,
            ))),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                locked: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                ..Default::default()
            },
        },
        TestStep {
            desc: "v3 precommits v1's proposal, it gets +2/3 precommits, decide it",
            input: Some(Input::Vote(new_signed_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v3.address,
            ))),
            expected_outputs: vec![Output::Decide(Round::new(0), proposal.message)],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Commit,
                locked: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                valid: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                decision: Some(RoundValue {
                    value: value.clone(),
                    round: Round::new(0),
                }),
                ..Default::default()
            },
        },
    ];

    run_steps(&mut driver, steps, sel.as_ref(), &vs);
}

#[test]
fn driver_steps_not_proposer_invalid() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([1, 2, 3]);

    // Proposer is v1, so we are not the proposer
    let (_my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let sel = Arc::new(FixedProposer::new(v1.address));
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let proposal = new_signed_proposal(
        Height::new(1),
        Round::new(0),
        value.clone(),
        Round::Nil,
        v1.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input: Some(Input::NewRound(
                Height::new(1),
                Round::new(0),
                proposal.validator_address,
            )),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::propose(Round::new(0)))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Receive an invalid proposal, prevote nil (v2)",
            input: Some(Input::Proposal(proposal.clone(), Validity::Invalid)),
            expected_outputs: vec![Output::Vote(Vote::new_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Nil,
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Receive our own prevote (v2)",
            input: None,
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc: "v1 prevotes its own proposal",
            input: Some(Input::Vote(new_signed_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v1.address,
            ))),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc:
                "v3 prevotes v1's proposal, we have a polka for any, schedule prevote timeout (v2)",
            input: Some(Input::Vote(new_signed_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v3.address,
            ))),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::prevote(Round::new(0)))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        TestStep {
            desc: "prevote timeout elapses, we precommit nil (v2)",
            input: Some(Input::TimeoutElapsed(Timeout::prevote(Round::new(0)))),
            expected_outputs: vec![Output::Vote(Vote::new_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Nil,
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                ..Default::default()
            },
        },
    ];

    run_steps(&mut driver, steps, sel.as_ref(), &vs);
}

#[test]
fn driver_steps_not_proposer_other_height() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2)] = make_validators([1, 2]);

    // Proposer is v1, so we are not the proposer
    let (_my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let sel = Arc::new(FixedProposer::new(v1.address));
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    // Proposal is for another height
    let proposal = new_signed_proposal(
        Height::new(2),
        Round::new(0),
        value.clone(),
        Round::Nil,
        v1.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input: Some(Input::NewRound(
                Height::new(1),
                Round::new(0),
                proposal.validator_address,
            )),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::propose(Round::new(0)))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Receive a proposal for another height, ignore it (v2)",
            input: Some(Input::Proposal(proposal.clone(), Validity::Invalid)),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
    ];

    let expected_error = Error::InvalidProposalHeight {
        proposal_height: Height::new(2),
        consensus_height: Height::new(1),
    };

    run_steps_failing(&mut driver, steps, expected_error, sel.as_ref(), &vs);
}

#[test]
fn driver_steps_not_proposer_other_round() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, sk2)] = make_validators([1, 2]);

    // Proposer is v1, so we are not the proposer
    let (_my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let sel = Arc::new(FixedProposer::new(v1.address));
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    // Proposal is for another round
    let proposal = new_signed_proposal(
        Height::new(1),
        Round::new(1),
        value.clone(),
        Round::Nil,
        v2.address,
    );

    let steps = vec![
        TestStep {
            desc: "Start round 0, we are not the proposer",
            input: Some(Input::NewRound(Height::new(1), Round::new(0), v1.address)),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::propose(Round::new(0)))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Receive a proposal for another round, ignore it (v2)",
            input: Some(Input::Proposal(proposal.clone(), Validity::Invalid)),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
    ];

    run_steps(&mut driver, steps, sel.as_ref(), &vs);
}

#[test]
fn driver_steps_not_proposer_timeout_multiple_rounds() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([1, 3, 1]);

    // Proposer is v1, so we, v3, are not the proposer
    let (_my_sk, my_addr) = (sk3, v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let sel = Arc::new(FixedProposer::new(v1.address));
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let steps = vec![
        // Start round 0, we, v3, are not the proposer
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer",
            input: Some(Input::NewRound(Height::new(1), Round::new(0), v1.address)),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::propose(Round::new(0)))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        // Receive a propose timeout, prevote nil (from v3)
        TestStep {
            desc: "Receive a propose timeout, prevote nil (v3)",
            input: Some(Input::TimeoutElapsed(Timeout::propose(Round::new(0)))),
            expected_outputs: vec![Output::Vote(Vote::new_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Nil,
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                round: Round::new(0),
                height: Height::new(1),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        // Receive our own prevote v3
        TestStep {
            desc: "Receive our own prevote v3",
            input: None,
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        // v1 prevotes its own proposal
        TestStep {
            desc: "v1 prevotes its own proposal",
            input: Some(Input::Vote(new_signed_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v1.address,
            ))),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        // v2 prevotes nil, we get +2/3 nil prevotes and precommit nil
        TestStep {
            desc: "v2 prevotes nil, we get +2/3 prevotes, precommit nil",
            input: Some(Input::Vote(new_signed_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Nil,
                v2.address,
            ))),
            expected_outputs: vec![Output::Vote(Vote::new_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Nil,
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                ..Default::default()
            },
        },
        // v3 receives its own precommit
        TestStep {
            desc: "v3 receives its own precommit",
            input: None,
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                ..Default::default()
            },
        },
        // v1 precommits its proposal
        TestStep {
            desc: "v1 precommits its proposal",
            input: Some(Input::Vote(new_signed_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v1.address,
            ))),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                ..Default::default()
            },
        },
        // v2 precommits nil
        TestStep {
            desc: "v2 precommits nil",
            input: Some(Input::Vote(new_signed_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Nil,
                v2.address,
            ))),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::precommit(Round::new(0)))],
            expected_round: Round::new(0),
            new_state: State {
                height: Height::new(1),
                round: Round::new(0),
                step: Step::Precommit,
                ..Default::default()
            },
        },
        // we receive a precommit timeout, start a new round
        TestStep {
            desc: "we receive a precommit timeout, start a new round",
            input: Some(Input::TimeoutElapsed(Timeout::precommit(Round::new(0)))),
            expected_outputs: vec![Output::NewRound(Height::new(1), Round::new(1))],
            expected_round: Round::new(1),
            new_state: State {
                height: Height::new(1),
                round: Round::new(1),
                step: Step::Unstarted,
                ..Default::default()
            },
        },
        TestStep {
            desc: "Start round 1, we are not the proposer",
            input: Some(Input::NewRound(Height::new(1), Round::new(1), v2.address)),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::propose(Round::new(1)))],
            expected_round: Round::new(1),
            new_state: State {
                height: Height::new(1),
                round: Round::new(1),
                step: Step::Propose,
                ..Default::default()
            },
        },
    ];

    run_steps(&mut driver, steps, sel.as_ref(), &vs);
}

// No value to propose
#[test]
fn driver_steps_no_value_to_propose() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let height = Height::new(1);
    let ctx = TestContext::new();

    // We are the proposer
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let outputs = driver
        .process(Input::NewRound(Height::new(1), Round::new(0), v1.address))
        .expect("execute succeeded");

    assert_eq!(
        outputs,
        vec!(
            Output::ScheduleTimeout(Timeout::new(Round::new(0), TimeoutKind::Propose)),
            Output::GetValue(
                Height::new(1),
                Round::new(0),
                Timeout::propose(Round::new(0))
            )
        )
    );
}

#[test]
fn driver_steps_proposer_not_found() {
    let [(v1, _sk1), (v2, sk2), (v3, _sk3)] = make_validators([1, 2, 3]);

    let (_my_sk, my_addr) = (sk2, v2.address);

    let height = Height::new(1);
    let ctx = TestContext::new();

    // Proposer is v1, which is not in the validator set
    let vs = ValidatorSet::new(vec![v2.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let output = driver.process(Input::NewRound(Height::new(1), Round::new(0), v1.address));
    assert_eq!(output, Err(Error::ProposerNotFound(v1.address)));
}

#[test]
fn driver_steps_validator_not_found() {
    let value = Value::new(9999);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([1, 2, 3]);

    let (_my_sk, my_addr) = (sk3.clone(), v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();

    // Proposer is v1
    // We omit v2 from the validator set
    let vs = ValidatorSet::new(vec![v1.clone(), v3.clone()]);

    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    // Start new height
    driver
        .process(Input::NewRound(Height::new(1), Round::new(0), v1.address))
        .expect("execute succeeded");

    // v2 prevotes some proposal, we cannot find it in the validator set => error
    let output = driver.process(Input::Vote(new_signed_prevote(
        Height::new(1),
        Round::new(0),
        NilOrVal::Val(value.id()),
        v2.address,
    )));

    assert_eq!(output, Err(Error::ValidatorNotFound(v2.address)));
}

#[test]
fn driver_steps_skip_round_skip_threshold() {
    let value = Value::new(9999);

    let sel = Arc::new(RotateProposer);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([1, 1, 1]);

    // Proposer is v1, so we, v3, are not the proposer
    let (_my_sk, my_addr) = (sk3, v3.address);

    let ctx = TestContext::new();
    let height = Height::new(1);

    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);
    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let steps = vec![
        // Start round 0, we, v3, are not the proposer
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer",
            input: Some(Input::NewRound(height, Round::new(0), v1.address)),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::propose(Round::new(0)))],
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        // Receive a propose timeout, prevote nil (from v3)
        TestStep {
            desc: "Receive a propose timeout, prevote nil (v3)",
            input: Some(Input::TimeoutElapsed(Timeout::propose(Round::new(0)))),
            expected_outputs: vec![Output::Vote(Vote::new_prevote(
                height,
                Round::new(0),
                NilOrVal::Nil,
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        // Receive our own prevote v3
        TestStep {
            desc: "Receive our own prevote v3",
            input: None,
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        // v1 prevotes its own proposal
        TestStep {
            desc: "v1 prevotes its own proposal in round 1",
            input: Some(Input::Vote(new_signed_prevote(
                height,
                Round::new(1),
                NilOrVal::Val(value.id()),
                v1.address,
            ))),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        // v2 prevotes v1 proposal in round 1, expected output is to move to next round
        TestStep {
            desc: "v2 prevotes v1 proposal, we get +1/3 messages from future round",
            input: Some(Input::Vote(new_signed_prevote(
                height,
                Round::new(1),
                NilOrVal::Val(value.id()),
                v2.address,
            ))),
            expected_outputs: vec![Output::NewRound(height, Round::new(1))],
            expected_round: Round::new(1),
            new_state: State {
                height,
                round: Round::new(1),
                ..Default::default()
            },
        },
    ];

    run_steps(&mut driver, steps, sel.as_ref(), &vs);
}

#[test]
fn driver_steps_skip_round_quorum_threshold() {
    let value = Value::new(9999);

    let sel = Arc::new(RotateProposer);

    let [(v1, _sk1), (v2, _sk2), (v3, sk3)] = make_validators([1, 2, 1]);

    // Proposer is v1, so we, v3, are not the proposer
    let (_my_sk, my_addr) = (sk3, v3.address);

    let height = Height::new(1);
    let ctx = TestContext::new();

    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);
    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let steps = vec![
        // Start round 0, we, v3, are not the proposer
        TestStep {
            desc: "Start round 0, we, v3, are not the proposer",
            input: Some(Input::NewRound(height, Round::new(0), v1.address)),
            expected_outputs: vec![Output::ScheduleTimeout(Timeout::propose(Round::new(0)))],
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Propose,
                ..Default::default()
            },
        },
        // Receive a propose timeout, prevote nil (from v3)
        TestStep {
            desc: "Receive a propose timeout, prevote nil (v3)",
            input: Some(Input::TimeoutElapsed(Timeout::propose(Round::new(0)))),
            expected_outputs: vec![Output::Vote(Vote::new_prevote(
                height,
                Round::new(0),
                NilOrVal::Nil,
                my_addr,
            ))],
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        // Receive our own prevote v3
        TestStep {
            desc: "Receive our own prevote v3",
            input: None,
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        // v1 prevotes its own proposal
        TestStep {
            desc: "v1 prevotes its own proposal in round 1",
            input: Some(Input::Vote(new_signed_prevote(
                height,
                Round::new(1),
                NilOrVal::Val(value.id()),
                v1.address,
            ))),
            expected_outputs: vec![],
            expected_round: Round::new(0),
            new_state: State {
                height,
                round: Round::new(0),
                step: Step::Prevote,
                ..Default::default()
            },
        },
        // v2 prevotes v1 proposal in round 1, expected output is to move to next round
        TestStep {
            desc: "v2 prevotes v1 proposal, we get +1/3 messages from future round",
            input: Some(Input::Vote(new_signed_prevote(
                height,
                Round::new(1),
                NilOrVal::Val(value.id()),
                v2.address,
            ))),
            expected_outputs: vec![Output::NewRound(height, Round::new(1))],
            expected_round: Round::new(1),
            new_state: State {
                height,
                round: Round::new(1),
                step: Step::Unstarted,
                ..Default::default()
            },
        },
    ];

    run_steps(&mut driver, steps, sel.as_ref(), &vs);
}

fn run_steps(
    driver: &mut Driver<TestContext>,
    steps: Vec<TestStep>,
    sel: &dyn ProposerSelector<TestContext>,
    vs: &ValidatorSet,
) {
    let mut input_from_prev_output = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let input = step
            .input
            .unwrap_or_else(|| input_from_prev_output.unwrap());

        let mut outputs = driver.process(input).expect("execute succeeded");

        assert_eq!(outputs, step.expected_outputs, "expected outputs");
        assert_eq!(driver.round(), step.expected_round, "expected round");
        assert_eq!(driver.round_state(), &step.new_state, "new state");

        input_from_prev_output = outputs
            .pop()
            .and_then(|input| output_to_input(input, sel, vs));
    }
}

fn run_steps_failing(
    driver: &mut Driver<TestContext>,
    steps: Vec<TestStep>,
    expected_error: Error<TestContext>,
    sel: &dyn ProposerSelector<TestContext>,
    vs: &ValidatorSet,
) {
    let mut input_from_prev_output = None;

    for step in steps {
        println!("Step: {}", step.desc);

        let input = step
            .input
            .unwrap_or_else(|| input_from_prev_output.unwrap());

        match driver.process(input) {
            Ok(mut outputs) => {
                assert_eq!(outputs, step.expected_outputs, "expected outputs");
                assert_eq!(driver.round(), step.expected_round, "expected round");
                assert_eq!(driver.round_state(), &step.new_state, "new state");

                input_from_prev_output = outputs
                    .pop()
                    .and_then(|input| output_to_input(input, sel, vs));
            }

            Err(error) => {
                assert_eq!(error, expected_error, "expected error");
                return;
            }
        }
    }
}
