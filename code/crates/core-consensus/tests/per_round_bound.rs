//! Per-`(height, round)` bounding for the full-proposal keeper and the WAL.
//!
//! A Byzantine source cannot grow the keeper — and therefore the WAL — without limit by flooding
//! distinct proposals or proposed values for a single `(height, round)`. Two kinds of entry are
//! exempt from the bound, both carrying a quorum of signatures that cannot be forged: values
//! delivered by the sync protocol, which come with verified commit certificates, and proposals or
//! values whose value already holds a polka certificate at that round.
//! Flooding does not defeat equivocation accountability: the two retained proposals are still
//! forwarded to the driver, which records the evidence.

use std::vec::Vec;

use arc_malachitebft_core_consensus::full_proposal::MAX_PROPOSALS_PER_ROUND;
use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, ProposedValue, Resumable, Resume, State, ValuePayload,
};
use malachitebft_core_types::{
    NilOrVal, PolkaCertificate, Round, SignedProposal, SignedVote, Validity, ValueOrigin,
};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Validator, ValidatorSet, Value, ValueId,
    Vote,
};

#[derive(Default)]
struct Captured {
    wal: Vec<Input<TestContext>>,
}

fn handle_effect(
    effect: Effect<TestContext>,
    cap: &mut Captured,
) -> Result<Resume<TestContext>, ()> {
    use Effect::*;
    Ok(match effect {
        VerifySignature(_, _, r) => r.resume_with(true),
        VerifyPolkaCertificate(_, _, _, r) => r.resume_with(Ok(())),
        VerifyRoundCertificate(_, _, _, r) => r.resume_with(Ok(())),
        VerifyCommitCertificate(_, _, _, r) => r.resume_with(Ok(())),
        SignVote(vote, r) => r.resume_with(SignedVote::new(vote, Signature::test())),
        SignProposal(proposal, r) => {
            r.resume_with(SignedProposal::new(proposal, Signature::test()))
        }
        ExtendVote(_, _, _, r) => r.resume_with(None),
        VerifyVoteExtension(_, _, _, _, _, _, r) => r.resume_with(Ok(())),
        WalAppend(_, input, r) => {
            cap.wal.push(input);
            r.resume_with(())
        }
        _ => Resume::Continue,
    })
}

fn prevote(addr: Address, round: u32, value_id: NilOrVal<ValueId>) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_prevote(Height::new(1), Round::new(round), value_id, addr),
        Signature::test(),
    )
}

/// A polka certificate for `value` at `round`, carrying a prevote from every validator.
fn polka_certificate(
    validators: &[Validator],
    round: u32,
    value: u64,
) -> PolkaCertificate<TestContext> {
    let value_id = Value::new(value).id();
    PolkaCertificate::new(
        Height::new(1),
        Round::new(round),
        value_id,
        validators
            .iter()
            .map(|v| prevote(v.address, round, NilOrVal::Val(value_id)))
            .collect(),
    )
}

fn make_state(
    validators: &[Validator],
    my_addr: Address,
    payload: ValuePayload,
) -> State<TestContext> {
    let vs = ValidatorSet::new(validators.to_vec());
    State::new(
        TestContext::new(),
        Height::new(1),
        vs,
        Params {
            address: my_addr,
            threshold_params: Default::default(),
            value_payload: payload,
            enabled: true,
        },
        1000,
        1000,
    )
}

fn proposed_value(proposer: Address, round: u32, value: u64) -> ProposedValue<TestContext> {
    ProposedValue {
        height: Height::new(1),
        round: Round::new(round),
        valid_round: Round::Nil,
        proposer,
        value: Value::new(value),
        validity: Validity::Valid,
    }
}

fn signed_proposal(proposer: Address, round: u32, value: u64) -> SignedProposal<TestContext> {
    SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(round),
            Value::new(value),
            Round::Nil,
            proposer,
        ),
        Signature::test(),
    )
}

fn proposed_values_in_wal(cap: &Captured) -> usize {
    cap.wal
        .iter()
        .filter(|i| matches!(i, Input::ProposedValue(_, _)))
        .count()
}

fn drive(state: &mut State<TestContext>, inputs: Vec<Input<TestContext>>) -> Captured {
    let metrics = Metrics::new();
    let mut cap = Captured::default();
    for input in inputs {
        let _: Result<(), Error<TestContext>> = process!(
            input: input,
            state: state,
            metrics: &metrics,
            with: e => handle_effect(e, &mut cap)
        );
    }
    cap
}

#[test]
fn consensus_proposed_values_are_bounded_per_round() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let me = validators[0].address;
    let proposer = validators[1].address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);

    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Flood distinct values for the same (height, round) from the consensus path.
    for value in 0..(MAX_PROPOSALS_PER_ROUND as u64 + 5) {
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, value),
            ValueOrigin::Consensus,
        ));
    }

    let cap = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&cap), MAX_PROPOSALS_PER_ROUND);
}

#[test]
fn sync_proposed_values_bypass_the_bound() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let me = validators[0].address;
    let proposer = validators[1].address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);

    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Fill the (height, round) bucket from the consensus path.
    for value in 0..(MAX_PROPOSALS_PER_ROUND as u64) {
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, value),
            ValueOrigin::Consensus,
        ));
    }
    // A further distinct value from the sync path must still be persisted.
    inputs.push(Input::ProposedValue(
        proposed_value(proposer, 0, 999),
        ValueOrigin::Sync,
    ));

    let cap = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&cap), MAX_PROPOSALS_PER_ROUND + 1);
}

#[test]
fn polka_certified_proposed_values_bypass_the_bound() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let me = validators[0].address;
    let proposer = validators[1].address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);

    let certified = MAX_PROPOSALS_PER_ROUND as u64 + 1;
    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Fill the (height, round) bucket from the consensus path.
    for value in 0..(MAX_PROPOSALS_PER_ROUND as u64) {
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, value),
            ValueOrigin::Consensus,
        ));
    }
    // A polka forms for a value the node does not hold, then that value arrives at its original
    // round — the round the certificate covers.
    inputs.push(Input::PolkaCertificate(polka_certificate(
        &validators,
        0,
        certified,
    )));
    inputs.push(Input::ProposedValue(
        proposed_value(proposer, 0, certified),
        ValueOrigin::Consensus,
    ));
    // A further uncertified value is still rejected.
    inputs.push(Input::ProposedValue(
        proposed_value(proposer, 0, 999),
        ValueOrigin::Consensus,
    ));

    let cap = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&cap), MAX_PROPOSALS_PER_ROUND + 1);
    assert!(state
        .get_proposed_value_by_id(Height::new(1), Round::new(0), &Value::new(certified).id())
        .is_some());
}

#[test]
fn polka_certified_value_is_admitted_when_redelivered_after_the_certificate() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let me = validators[0].address;
    let proposer = validators[1].address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);

    let certified = MAX_PROPOSALS_PER_ROUND as u64 + 1;
    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Fill the (height, round) bucket from the consensus path.
    for value in 0..(MAX_PROPOSALS_PER_ROUND as u64) {
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, value),
            ValueOrigin::Consensus,
        ));
    }
    // The value arrives ahead of any certificate for it, so the cap rejects it.
    inputs.push(Input::ProposedValue(
        proposed_value(proposer, 0, certified),
        ValueOrigin::Consensus,
    ));

    let cap = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&cap), MAX_PROPOSALS_PER_ROUND);
    assert!(state
        .get_proposed_value_by_id(Height::new(1), Round::new(0), &Value::new(certified).id())
        .is_none());

    // The certificate is retained once it arrives, so the next delivery of the same value is
    // admitted.
    let cap = drive(
        &mut state,
        vec![
            Input::PolkaCertificate(polka_certificate(&validators, 0, certified)),
            Input::ProposedValue(
                proposed_value(proposer, 0, certified),
                ValueOrigin::Consensus,
            ),
        ],
    );

    assert_eq!(proposed_values_in_wal(&cap), 1);
    assert!(state
        .get_proposed_value_by_id(Height::new(1), Round::new(0), &Value::new(certified).id())
        .is_some());
}

#[test]
fn polka_certified_proposals_bypass_the_bound() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();

    // Run as a non-proposer so the node does not try to build its own proposal at round 0.
    let proposer = *make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalOnly,
    )
    .get_proposer(Height::new(1), Round::new(0));
    let me = validators
        .iter()
        .find(|v| v.address != proposer)
        .expect("a non-proposer validator")
        .address;

    let mut state = make_state(&validators, me, ValuePayload::ProposalOnly);

    let certified = MAX_PROPOSALS_PER_ROUND as u64;
    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Fill the (height, round) bucket with distinct proposals from the round-0 proposer.
    inputs.extend(
        (0..(MAX_PROPOSALS_PER_ROUND as u64))
            .map(|value| Input::Proposal(signed_proposal(proposer, 0, value))),
    );
    inputs.push(Input::PolkaCertificate(polka_certificate(
        &validators,
        0,
        certified,
    )));
    inputs.push(Input::Proposal(signed_proposal(proposer, 0, certified)));
    // A further uncertified proposal is still rejected.
    inputs.push(Input::Proposal(signed_proposal(proposer, 0, certified + 1)));

    let _ = drive(&mut state, inputs);

    let height = Height::new(1);
    let round = Round::new(0);
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(certified))
        .is_some());
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(certified + 1))
        .is_none());
    // The two entries stored before the certificate are retained alongside it.
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(0))
        .is_some());
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(1))
        .is_some());
}

#[test]
fn flooding_proposals_records_at_most_one_evidence_pair() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let vs = ValidatorSet::new(validators.clone());

    // Run as a non-proposer so the node does not try to build its own proposal at round 0.
    let proposer = *make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalOnly,
    )
    .get_proposer(Height::new(1), Round::new(0));
    let me = validators
        .iter()
        .find(|v| v.address != proposer)
        .expect("a non-proposer validator")
        .address;

    let mut state = make_state(&validators, me, ValuePayload::ProposalOnly);

    // Enter height 1, then have the round-0 proposer flood distinct proposals. In proposal-only
    // mode each is paired with a synthesized value, becoming a full proposal forwarded to the
    // driver.
    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        vs,
        false,
        None,
        Default::default(),
    )];
    inputs.extend(
        (0..(MAX_PROPOSALS_PER_ROUND as u64 + 8))
            .map(|value| Input::Proposal(signed_proposal(proposer, 0, value))),
    );
    let _ = drive(&mut state, inputs);

    // Only MAX distinct proposals are retained; the rest are dropped at the pre-gate.
    let height = Height::new(1);
    let round = Round::new(0);
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(0))
        .is_some());
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(1))
        .is_some());
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(2))
        .is_none());

    // The two retained proposals equivocate; the driver records exactly one evidence pair.
    let evidence = state.driver.take_proposal_evidence();
    assert_eq!(
        evidence
            .get(&proposer)
            .map(|pairs| pairs.len())
            .unwrap_or(0),
        1
    );
}
