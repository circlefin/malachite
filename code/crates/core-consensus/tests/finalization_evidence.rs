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
    )
}

fn handle_effect(effect: Effect<TestContext>) -> Result<Resume<TestContext>, ()> {
    use Effect::*;
    Ok(match effect {
        VerifySignature(_, _, r) => r.resume_with(true),
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

    run(process!(
        input: Input::StartHeight(Height::new(1), vs, false, None),
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

    state.finalization_period = true;
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
