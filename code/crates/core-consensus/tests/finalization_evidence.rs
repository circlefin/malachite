use std::time::Duration;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, ProposedValue, Resumable, Resume, State,
};
use malachitebft_core_types::{
    NilOrVal, Round, SignedProposal, SignedVote, Validity, ValueOrigin, ValuePayload,
};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Validator, ValidatorSet, Value, Vote,
};

fn run(r: Result<(), Error<TestContext>>) {
    drop(r);
}

fn make_state(validators: &[Validator], my_addr: Address) -> State<TestContext> {
    let vs = ValidatorSet::new(validators.to_vec());
    State::new(
        TestContext::new(),
        Height::new(1),
        vs.clone(),
        Params {
            address: my_addr,
            threshold_params: Default::default(),
            value_payload: ValuePayload::ProposalOnly,
            enabled: true,
        },
        1000,
        1000,
    )
}

fn handle_effect(effect: Effect<TestContext>) -> Result<Resume<TestContext>, ()> {
    use Effect::*;
    Ok(match effect {
        VerifySignature(_, _, r) => r.resume_with(true),
        VerifyCommitCertificate(_, _, _, r) => r.resume_with(Ok(())),
        SignVote(vote, r) => r.resume_with(SignedVote::new(vote, Signature::test())),
        SignProposal(proposal, r) => {
            r.resume_with(SignedProposal::new(proposal, Signature::test()))
        }
        _ => Resume::Continue,
    })
}

fn drive_to_finalization(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    validators: &[Validator],
    proposer: Address,
    value: Value,
) {
    let vs = ValidatorSet::new(validators.to_vec());

    // Large target_time: decide() enters the finalization window and the
    // finalize_height timeout does not fire during the test.
    let target_time = Some(Duration::from_secs(3600));

    run(process!(
        input: Input::StartHeight(Height::new(1), vs, false, target_time, Default::default()),
        state: state,
        metrics: metrics,
        with: effect => handle_effect(effect)
    ));

    let proposal = SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(0),
            value.clone(),
            Round::Nil,
            proposer,
        ),
        Signature::test(),
    );
    run(process!(
        input: Input::Proposal(proposal),
        state: state,
        metrics: metrics,
        with: effect => handle_effect(effect)
    ));

    run(process!(
        input: Input::ProposedValue(
            ProposedValue {
                height: Height::new(1),
                round: Round::new(0),
                valid_round: Round::Nil,
                proposer,
                value: value.clone(),
                validity: Validity::Valid,
            },
            ValueOrigin::Consensus,
        ),
        state: state,
        metrics: metrics,
        with: effect => handle_effect(effect)
    ));

    for v in validators {
        let prevote = SignedVote::new(
            Vote::new_prevote(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v.address,
            ),
            Signature::test(),
        );
        run(process!(
            input: Input::Vote(prevote),
            state: state,
            metrics: metrics,
            with: effect => handle_effect(effect)
        ));
    }

    for v in validators {
        let precommit = SignedVote::new(
            Vote::new_precommit(
                Height::new(1),
                Round::new(0),
                NilOrVal::Val(value.id()),
                v.address,
            ),
            Signature::test(),
        );
        run(process!(
            input: Input::Vote(precommit),
            state: state,
            metrics: metrics,
            with: effect => handle_effect(effect)
        ));
    }

    assert!(state.finalization_period);
    assert!(state.driver.step_is_commit());
}

fn equivocating_proposal(addr: Address) -> Input<TestContext> {
    Input::Proposal(SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(0),
            Value::new(100),
            Round::Nil,
            addr,
        ),
        Signature::test(),
    ))
}

fn equivocating_prevote(addr: Address) -> Input<TestContext> {
    Input::Vote(SignedVote::new(
        Vote::new_prevote(
            Height::new(1),
            Round::new(0),
            NilOrVal::Val(Value::new(100).id()),
            addr,
        ),
        Signature::test(),
    ))
}

fn equivocating_precommit(addr: Address) -> Input<TestContext> {
    Input::Vote(SignedVote::new(
        Vote::new_precommit(
            Height::new(1),
            Round::new(0),
            NilOrVal::Val(Value::new(100).id()),
            addr,
        ),
        Signature::test(),
    ))
}

fn vote_evidence_count(state: &State<TestContext>, addr: Address) -> usize {
    state
        .driver
        .votes()
        .evidence()
        .get(&addr)
        .map(|v: &Vec<_>| v.len())
        .unwrap_or(0)
}

fn proposal_evidence_count(state: &State<TestContext>, addr: Address) -> usize {
    state
        .driver
        .proposals()
        .evidence()
        .get(&addr)
        .map(|v: &Vec<_>| v.len())
        .unwrap_or(0)
}

struct TestCase {
    name: &'static str,
    make_input: fn(Address) -> Input<TestContext>,
    get_evidence_count: fn(&State<TestContext>, Address) -> usize,
    expected: usize,
}

#[test]
fn same_value_proposal_with_different_pol_round_is_recorded_as_evidence() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let proposer = validators[0].address;
    let value = Value::new(9999);
    let metrics = Metrics::new();

    let mut state = make_state(&validators, proposer);
    drive_to_finalization(&mut state, &metrics, &validators, proposer, value.clone());

    // The first proposal — fed inside `drive_to_finalization` — has `pol_round = Nil`.
    // Feed a second proposal from the same proposer for the same `(height, round, value)`
    // but with a different `pol_round`. The two proposals share a value id, so they
    // are filtered as a "same value" pair at the full-proposal keeper; evidence must
    // still be recorded.
    let equivocating = Input::Proposal(SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(0),
            value,
            Round::new(0),
            proposer,
        ),
        Signature::test(),
    ));

    run(process!(
        input: equivocating,
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));

    assert_eq!(
        proposal_evidence_count(&state, proposer),
        1,
        "proposal equivocation should be detected even when the two proposals \
         carry the same value id"
    );
}

#[test]
fn repeated_same_value_equivocating_proposal_is_deduplicated() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let proposer = validators[0].address;
    let value = Value::new(9999);
    let metrics = Metrics::new();

    let mut state = make_state(&validators, proposer);
    drive_to_finalization(&mut state, &metrics, &validators, proposer, value.clone());

    let equivocating = SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(0),
            value,
            Round::new(0),
            proposer,
        ),
        Signature::test(),
    );

    for _ in 0..3 {
        run(process!(
            input: Input::Proposal(equivocating.clone()),
            state: &mut state,
            metrics: &metrics,
            with: effect => handle_effect(effect)
        ));
    }

    assert_eq!(
        proposal_evidence_count(&state, proposer),
        1,
        "receiving the same equivocating proposal multiple times should still record \
         exactly one evidence entry"
    );
}

#[test]
fn equivocation_detection_in_finalization_period() {
    let tests = vec![
        TestCase {
            name: "prevote",
            make_input: equivocating_prevote,
            get_evidence_count: vote_evidence_count,
            expected: 1,
        },
        TestCase {
            name: "precommit",
            make_input: equivocating_precommit,
            get_evidence_count: vote_evidence_count,
            expected: 1,
        },
        TestCase {
            name: "proposal",
            make_input: equivocating_proposal,
            get_evidence_count: proposal_evidence_count,
            expected: 1,
        },
    ];

    for test in tests {
        println!("Testing: {}", test.name);

        let validators: Vec<_> = make_validators([1, 1, 1])
            .into_iter()
            .map(|(v, _)| v)
            .collect();
        let proposer = validators[0].address;
        let value = Value::new(9999);
        let metrics = Metrics::new();

        let mut state = make_state(&validators, proposer);
        drive_to_finalization(&mut state, &metrics, &validators, proposer, value);

        // All equivocations come from the proposer
        let input = (test.make_input)(proposer);

        run(process!(
            input: input,
            state: &mut state,
            metrics: &metrics,
            with: effect => handle_effect(effect)
        ));

        let count = (test.get_evidence_count)(&state, proposer);

        assert_eq!(
            count, test.expected,
            "{} equivocation should be detected during finalization",
            test.name
        );
    }
}

#[test]
fn start_height_during_finalization_period_flushes_finalize_with_evidence() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let proposer = validators[0].address;
    let value = Value::new(9999);
    let metrics = Metrics::new();

    let mut state = make_state(&validators, proposer);
    drive_to_finalization(&mut state, &metrics, &validators, proposer, value);

    // Accumulate equivocation evidence during the finalization window.
    run(process!(
        input: equivocating_prevote(proposer),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));
    assert_eq!(vote_evidence_count(&state, proposer), 1);
    assert!(state.finalization_period);

    // Advance to the next height while the finalization window is still open.
    let new_height = Height::new(2);
    let new_vs = ValidatorSet::new(validators.clone());
    let mut finalize_evidence_empty = None;

    run(process!(
        input: Input::StartHeight(new_height, new_vs, false, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => {
            if let Effect::Finalize(_, _, evidence, _) = &effect {
                finalize_evidence_empty = Some(evidence.is_empty());
            }
            handle_effect(effect)
        }
    ));

    assert_eq!(
        finalize_evidence_empty,
        Some(false),
        "StartHeight during the finalization window must emit Finalize with non-empty evidence"
    );
    assert!(!state.finalization_period);
    assert_eq!(state.height(), new_height);
}
